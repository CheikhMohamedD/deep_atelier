//! Assemblage du projet Next.js : pages, layouts, composants, îlots clients, thème, images de
//! remplissage, configuration (reprise du gabarit `create-next-app` 16.4).

use std::collections::BTreeMap;

use ir::{BindableField, Component, Document, FontFamily, Layout, NodeId, Page};

use crate::doc::{Mark, concat, hardline, join, text};
use crate::elements::{Emitter, Scope, prop_literal};
use crate::jsx::{AttrValue, Element, Expr, Jsx, string_literal};
use crate::module::{Export, Imports, Item, Member, Module, Param, Statement, TypeExpr, Value};
use crate::names::{google_font_function, pascal_case, route_folder};
use crate::plan::{Island, Placeholder, Plan};
use crate::theme::{font_variable, globals_css};
use crate::{File, FileContents, Mode, Project, SourceRange};

/// Versions épinglées des dépendances du projet exporté.
pub const NEXT_VERSION: &str = "16.4.0";
pub const REACT_VERSION: &str = "19.3.0";
pub const LUCIDE_VERSION: &str = "1.52.0";
pub const TAILWIND_MERGE_VERSION: &str = "^3.7.0";

/// Compile un document en projet.
pub fn compile(doc: &Document, mode: Mode) -> Project {
    let plan = Plan::new(doc);
    let edit = mode == Mode::Edit;
    let mut files: Vec<File> = Vec::new();
    let mut uses_twmerge = false;
    let mut uses_lucide = false;

    files.push(text_file("app/globals.css", globals_css(doc)));
    files.push(root_layout(doc, &plan));
    // Sans favicon choisi, une icône par défaut évite la requête `/favicon.ico` en 404.
    if doc.settings.favicon.is_none() {
        files.push(text_file("app/icon.svg", default_icon(doc)));
    }

    // Layouts utilisés par au moins une page.
    let mut used_layouts: Vec<&Layout> = Vec::new();
    for page in &doc.pages {
        if let Some(layout) = page.layout.as_ref().and_then(|l| doc.layout(l))
            && !used_layouts.iter().any(|l| l.id == layout.id)
        {
            used_layouts.push(layout);
        }
    }
    for layout in used_layouts {
        let (file, twmerge, lucide) = layout_file(doc, &plan, layout, edit);
        uses_twmerge |= twmerge;
        uses_lucide |= lucide;
        files.push(file);
    }
    for page in &doc.pages {
        let (file, twmerge, lucide) = page_file(doc, &plan, page, edit);
        uses_twmerge |= twmerge;
        uses_lucide |= lucide;
        files.push(file);
    }
    for component in &doc.components {
        let Some(component_plan) = plan.components.get(&component.id) else {
            continue;
        };
        let (file, twmerge, lucide) = component_file(doc, &plan, component, component_plan, edit);
        uses_twmerge |= twmerge;
        uses_lucide |= lucide;
        files.push(file);
    }
    for island in plan.islands.values() {
        let (file, twmerge, lucide) = island_file(doc, &plan, island, edit);
        uses_twmerge |= twmerge;
        uses_lucide |= lucide;
        files.push(file);
    }
    for (node, name) in &plan.raw_clients {
        if let Some(file) = raw_client_file(doc, node, name) {
            files.push(file);
        }
    }
    for placeholder in plan.placeholders.values() {
        files.push(text_file(
            &format!("public{}", placeholder.path),
            placeholder_svg(placeholder),
        ));
    }
    for asset in &doc.assets {
        if let Some(path) = plan.assets.get(&asset.id) {
            files.push(File {
                path: format!("public{path}"),
                contents: FileContents::Asset(asset.id.clone()),
                source_map: Vec::new(),
            });
        }
    }
    files.extend(scaffold(doc, &plan, uses_twmerge, uses_lucide));
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Project { files }
}

fn text_file(path: &str, contents: String) -> File {
    File {
        path: path.to_owned(),
        contents: FileContents::Text(contents),
        source_map: Vec::new(),
    }
}

/// Fichier d'un module imprimé, avec ses plages de nœuds.
fn module_file(path: String, module: &Module) -> File {
    let printed = module.print();
    let mut starts: BTreeMap<NodeId, usize> = BTreeMap::new();
    let mut source_map = Vec::new();
    for (mark, offset) in printed.marks {
        match mark {
            Mark::Start(id) => {
                starts.insert(id, offset);
            }
            Mark::End(id) => {
                if let Some(start) = starts.remove(&id) {
                    source_map.push(SourceRange {
                        node: id,
                        start,
                        end: offset,
                    });
                }
            }
        }
    }
    source_map.sort_by_key(|r| (r.start, r.end));
    File {
        path,
        contents: FileContents::Text(printed.text),
        source_map,
    }
}

/// Dossier `app/` d'une page : groupe de son layout, puis sa route.
fn page_folder(doc: &Document, page: &Page) -> String {
    let mut folder = String::from("app");
    if let Some(layout) = page.layout.as_ref().and_then(|l| doc.layout(l)) {
        folder.push_str(&format!("/({})", layout.name));
    }
    let route = route_folder(&page.route);
    if !route.is_empty() {
        folder.push('/');
        folder.push_str(&route);
    }
    folder
}

fn page_metadata(page: &Page, plan: &Plan) -> Vec<(String, Value)> {
    let title = if page.seo.title.trim().is_empty() {
        &page.name
    } else {
        &page.seo.title
    };
    let mut props = vec![("title".to_owned(), Value::Str(title.clone()))];
    if !page.seo.description.trim().is_empty() {
        props.push(("description".to_owned(), Value::Str(page.seo.description.clone())));
    }
    if let Some(image) = page.seo.og_image.as_ref().and_then(|a| plan.assets.get(a)) {
        props.push((
            "openGraph".to_owned(),
            Value::Object(vec![(
                "images".to_owned(),
                Value::Array(vec![Value::Str(image.clone())]),
            )]),
        ));
    }
    props
}

fn function_item(
    name: String,
    export: Export,
    params: Option<(Vec<Param>, String)>,
    emitter_statements: Vec<Statement>,
    jsx: Jsx,
) -> Item {
    let mut body = emitter_statements;
    body.push(Statement::Return(jsx));
    Item::Function {
        export,
        name,
        params,
        body,
    }
}

fn page_file(doc: &Document, plan: &Plan, page: &Page, edit: bool) -> (File, bool, bool) {
    let mut emitter = Emitter::new(doc, plan, Scope::Route, edit);
    let jsx = root_jsx(&mut emitter, doc, &page.root);
    let mut imports = std::mem::take(&mut emitter.imports);
    imports.types("next", "Metadata");
    let lucide = uses(&imports, "lucide-react");
    let mut items = std::mem::take(&mut emitter.items);
    items.push(Item::Const {
        export: true,
        name: "metadata".to_owned(),
        annotation: Some("Metadata".to_owned()),
        props: page_metadata(page, plan),
        as_const: false,
    });
    let name = format!("{}Page", non_empty_pascal(&page.name, "Home"));
    items.push(function_item(
        name,
        Export::Default,
        None,
        emitter.statements.clone(),
        jsx,
    ));
    let module = Module {
        client: false,
        imports,
        items,
    };
    let path = format!("{}/page.tsx", page_folder(doc, page));
    (module_file(path, &module), emitter.uses_twmerge, lucide)
}

fn layout_file(doc: &Document, plan: &Plan, layout: &Layout, edit: bool) -> (File, bool, bool) {
    let mut emitter = Emitter::new(doc, plan, Scope::Route, edit);
    let jsx = root_jsx(&mut emitter, doc, &layout.root);
    let imports = std::mem::take(&mut emitter.imports);
    let lucide = uses(&imports, "lucide-react");
    let mut items = std::mem::take(&mut emitter.items);
    let name = format!("{}Layout", non_empty_pascal(&layout.name, "Site"));
    items.push(function_item(
        name,
        Export::Default,
        Some((vec![Param::new("children")], "LayoutProps<\"/\">".to_owned())),
        emitter.statements.clone(),
        jsx,
    ));
    let module = Module {
        client: false,
        imports,
        items,
    };
    let path = format!("app/({})/layout.tsx", layout.name);
    (module_file(path, &module), emitter.uses_twmerge, lucide)
}

/// JSX d'une racine de page ou de layout : fragment si la racine n'a ni rôle ni attribut.
fn root_jsx(emitter: &mut Emitter<'_>, doc: &Document, root: &NodeId) -> Jsx {
    let jsx = emitter.tree(root);
    let Some(node) = doc.node(root) else { return jsx };
    let generic = node
        .kind
        .container()
        .is_some_and(|(_, p)| p.role == ir::ContainerRole::Generic);
    match jsx {
        Jsx::Element(mut element) if generic && element.attrs.is_empty() && element.name == "div" => {
            // Un seul enfant élément : pas de fragment autour.
            if element.children.len() == 1 && matches!(element.children[0], Jsx::Element(_)) {
                element.children.remove(0)
            } else {
                Jsx::Fragment(element.children)
            }
        }
        other => other,
    }
}

fn uses(imports: &Imports, module: &str) -> bool {
    imports.has_module(module)
}

fn non_empty_pascal(text: &str, fallback: &str) -> String {
    let name = pascal_case(text);
    if name.is_empty() { fallback.to_owned() } else { name }
}

fn component_file(
    doc: &Document,
    plan: &Plan,
    component: &Component,
    component_plan: &crate::plan::ComponentPlan,
    edit: bool,
) -> (File, bool, bool) {
    let mut emitter = Emitter::new(doc, plan, Scope::Component(component, component_plan), edit);
    let jsx = emitter.tree(&component.root);
    let mut imports = std::mem::take(&mut emitter.imports);
    let lucide = uses(&imports, "lucide-react");
    let props_type = format!("{}Props", component_plan.name);

    let mut members = Vec::new();
    let mut params = Vec::new();
    for prop in &component_plan.props {
        let ty = match prop.field {
            BindableField::Visible => "boolean",
            _ => "string",
        };
        members.push(Member {
            name: prop.name.clone(),
            optional: true,
            ty: TypeExpr::Raw(ty.to_owned()),
        });
        params.push(Param::with_default(
            prop.name.clone(),
            prop_literal(plan, &prop.default),
        ));
    }
    for axis in &component.variants {
        members.push(Member {
            name: axis.name.clone(),
            optional: true,
            ty: TypeExpr::Union(axis.options.iter().map(|o| o.name.clone()).collect()),
        });
        params.push(Param::with_default(axis.name.clone(), string_literal(&axis.default)));
    }
    if component_plan.id_prop {
        members.push(Member {
            name: "id".to_owned(),
            optional: true,
            ty: TypeExpr::Raw("string".to_owned()),
        });
        params.push(Param::new("id"));
    }
    if component_plan.class_name {
        members.push(Member {
            name: "className".to_owned(),
            optional: true,
            ty: TypeExpr::Raw("string".to_owned()),
        });
        params.push(Param::new("className"));
    }
    for slot in &component_plan.slots {
        imports.types("react", "ReactNode");
        members.push(Member {
            name: slot.clone(),
            optional: true,
            ty: TypeExpr::Raw("ReactNode".to_owned()),
        });
        params.push(Param::new(slot.clone()));
    }
    // Attributs `aria-*` et `data-*` d'une instance, transmis à la racine.
    if component_plan.rest {
        imports.types("react", "AriaAttributes");
        params.push(Param::rest("rest"));
    }
    let mut items = Vec::new();
    if !members.is_empty() || component_plan.rest {
        items.push(Item::TypeAlias {
            name: props_type.clone(),
            intersection: component_plan.rest.then(|| "AriaAttributes".to_owned()),
            members,
        });
    }
    items.extend(std::mem::take(&mut emitter.items));
    let params = (!params.is_empty()).then_some((params, props_type));
    items.push(function_item(
        component_plan.name.clone(),
        Export::Named,
        params,
        emitter.statements.clone(),
        jsx,
    ));
    let module = Module {
        client: component_plan.client,
        imports,
        items,
    };
    let path = format!("components/{}.tsx", component_plan.name);
    (module_file(path, &module), emitter.uses_twmerge, lucide)
}

fn island_file(doc: &Document, plan: &Plan, island: &Island, edit: bool) -> (File, bool, bool) {
    let mut emitter = Emitter::new(doc, plan, Scope::Island(island), edit);
    let jsx = emitter.tree(&island.root);
    let mut imports = std::mem::take(&mut emitter.imports);
    let lucide = uses(&imports, "lucide-react");
    let params = if island.page_slot {
        imports.types("react", "ReactNode");
        Some((vec![Param::new("children")], "{ children: ReactNode }".to_owned()))
    } else {
        None
    };
    let mut items = std::mem::take(&mut emitter.items);
    items.push(function_item(
        island.name.clone(),
        Export::Named,
        params,
        emitter.statements.clone(),
        jsx,
    ));
    let module = Module {
        client: true,
        imports,
        items,
    };
    let path = format!("components/{}.tsx", island.name);
    (module_file(path, &module), emitter.uses_twmerge, lucide)
}

fn raw_client_file(doc: &Document, node: &NodeId, name: &str) -> Option<File> {
    let ir::NodeKind::RawCode(raw) = &doc.node(node)?.kind else {
        return None;
    };
    let mut imports = Imports::default();
    for import in &raw.imports {
        if let Some(default) = &import.default {
            imports.default_import(&import.module, default);
        }
        for named in &import.named {
            imports.named(&import.module, named);
        }
    }
    let module = Module {
        client: true,
        imports,
        items: vec![function_item(
            name.to_owned(),
            Export::Named,
            None,
            Vec::new(),
            Jsx::Raw(raw.code.clone()),
        )],
    };
    Some(module_file(format!("components/raw/{name}.tsx"), &module))
}

fn root_layout(doc: &Document, plan: &Plan) -> File {
    let mut imports = Imports::default();
    imports.types("next", "Metadata");
    imports.side_effect("./globals.css");
    let mut items = Vec::new();
    let mut variables = Vec::new();
    for font in &doc.tokens.fonts {
        let FontFamily::Google { family, weights } = &font.family else {
            continue;
        };
        let function = google_font_function(family);
        imports.named("next/font/google", &function);
        let var = crate::names::camel_case(family);
        let mut lines = vec![
            text(format!("const {var} = {function}({{")),
            text(format!("  variable: {},", string_literal(&font_variable(family)))),
            text("  subsets: [\"latin\"],".to_owned()),
        ];
        if !weights.is_empty() {
            let list = weights
                .iter()
                .map(|w| string_literal(&w.to_string()))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(text(format!("  weight: [{list}],")));
        }
        lines.push(text("});".to_owned()));
        items.push(Item::Raw(join(&hardline(), lines)));
        variables.push(format!("${{{var}.variable}}"));
    }
    let mut metadata = vec![("title".to_owned(), Value::Str(doc.settings.site_name.clone()))];
    if let Some(icon) = doc.settings.favicon.as_ref().and_then(|a| plan.assets.get(a)) {
        metadata.push((
            "icons".to_owned(),
            Value::Object(vec![("icon".to_owned(), Value::Str(icon.clone()))]),
        ));
    }
    items.push(Item::Const {
        export: true,
        name: "metadata".to_owned(),
        annotation: Some("Metadata".to_owned()),
        props: metadata,
        as_const: false,
    });
    let class_name = if variables.is_empty() {
        AttrValue::Str("antialiased".to_owned())
    } else {
        AttrValue::Expr(Expr::Inline(format!("`{} antialiased`", variables.join(" "))))
    };
    let html = Element::new("html")
        .string("lang", doc.settings.lang.clone())
        .attr("className", class_name)
        .child(Jsx::Element(
            Element::new("body").child(Jsx::Expr(Expr::Atom("children".to_owned()))),
        ));
    items.push(function_item(
        "RootLayout".to_owned(),
        Export::Default,
        Some((vec![Param::new("children")], "LayoutProps<\"/\">".to_owned())),
        Vec::new(),
        Jsx::Element(html),
    ));
    module_file(
        "app/layout.tsx".to_owned(),
        &Module {
            client: false,
            imports,
            items,
        },
    )
}

/// Icône par défaut : initiale du site sur la couleur `primary`.
fn default_icon(doc: &Document) -> String {
    let token = |name: &str, fallback: &str| {
        doc.tokens
            .colors
            .iter()
            .find(|c| c.name.as_str() == name)
            .map_or_else(|| fallback.to_owned(), |c| c.light.to_string())
    };
    let initial = crate::names::pascal_case(&doc.settings.site_name)
        .chars()
        .next()
        .unwrap_or('S');
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"32\" height=\"32\" viewBox=\"0 0 32 32\">\n  <rect width=\"32\" height=\"32\" rx=\"8\" fill=\"{}\" />\n  <text x=\"50%\" y=\"50%\" dominant-baseline=\"central\" text-anchor=\"middle\" font-family=\"system-ui, sans-serif\" font-size=\"18\" font-weight=\"700\" fill=\"{}\">{initial}</text>\n</svg>\n",
        token("primary", "#171717"),
        token("primary-foreground", "#fafafa"),
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Image de remplissage : rectangle neutre et son libellé.
pub(crate) fn placeholder_svg(placeholder: &Placeholder) -> String {
    let Placeholder {
        label, width, height, ..
    } = placeholder;
    let size = (width.min(height) / 12).max(16);
    let label = xml_escape(label);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" role=\"img\" aria-label=\"{label}\">\n  <rect width=\"{width}\" height=\"{height}\" fill=\"#e5e5e5\" />\n  <text x=\"50%\" y=\"50%\" dominant-baseline=\"middle\" text-anchor=\"middle\" font-family=\"system-ui, sans-serif\" font-size=\"{size}\" fill=\"#737373\">{label}</text>\n</svg>\n"
    )
}

/// Nom de paquet npm tiré du nom du site.
fn package_name(doc: &Document) -> String {
    let name = crate::names::kebab_case(&doc.settings.site_name);
    if name.is_empty() { "site".to_owned() } else { name }
}

fn scaffold(doc: &Document, plan: &Plan, twmerge: bool, lucide: bool) -> Vec<File> {
    let mut dependencies = vec![
        ("next".to_owned(), NEXT_VERSION.to_owned()),
        ("react".to_owned(), REACT_VERSION.to_owned()),
        ("react-dom".to_owned(), REACT_VERSION.to_owned()),
    ];
    if lucide {
        dependencies.push(("lucide-react".to_owned(), LUCIDE_VERSION.to_owned()));
    }
    if twmerge {
        dependencies.push(("tailwind-merge".to_owned(), TAILWIND_MERGE_VERSION.to_owned()));
    }
    dependencies.sort();
    let deps = dependencies
        .iter()
        .map(|(n, v)| format!("    \"{n}\": \"{v}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    let package = format!(
        "{{\n  \"name\": \"{}\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"scripts\": {{\n    \"dev\": \"next dev\",\n    \"build\": \"next build\",\n    \"start\": \"next start\",\n    \"lint\": \"eslint\"\n  }},\n  \"dependencies\": {{\n{deps}\n  }},\n  \"devDependencies\": {{\n    \"@tailwindcss/turbopack\": \"^4\",\n    \"@types/node\": \"^20\",\n    \"@types/react\": \"^19\",\n    \"@types/react-dom\": \"^19\",\n    \"eslint\": \"^9\",\n    \"eslint-config-next\": \"{NEXT_VERSION}\",\n    \"tailwindcss\": \"^4\",\n    \"typescript\": \"^5\"\n  }}\n}}\n",
        package_name(doc)
    );

    let mut config_props = vec![
        ("cacheComponents".to_owned(), Value::Raw("true".to_owned())),
        ("partialPrefetching".to_owned(), Value::Raw("true".to_owned())),
    ];
    if !plan.remote_hosts.is_empty() {
        let patterns = plan
            .remote_hosts
            .iter()
            .map(|(protocol, hostname)| {
                Value::Object(vec![
                    ("protocol".to_owned(), Value::Str(protocol.clone())),
                    ("hostname".to_owned(), Value::Str(hostname.clone())),
                ])
            })
            .collect();
        config_props.push((
            "images".to_owned(),
            Value::Expanded(vec![("remotePatterns".to_owned(), Value::Array(patterns))]),
        ));
    }
    config_props.push((
        "turbopack".to_owned(),
        Value::Expanded(vec![(
            "rules".to_owned(),
            Value::Expanded(vec![(
                "*.css".to_owned(),
                Value::Expanded(vec![
                    (
                        "loaders".to_owned(),
                        Value::Array(vec![Value::Str("@tailwindcss/turbopack".to_owned())]),
                    ),
                    ("as".to_owned(), Value::Str("*.css".to_owned())),
                ]),
            )]),
        )]),
    ));
    let mut config_imports = Imports::default();
    config_imports.types("next", "NextConfig");
    let config = Module {
        client: false,
        imports: config_imports,
        items: vec![
            Item::Const {
                export: false,
                name: "nextConfig".to_owned(),
                annotation: Some("NextConfig".to_owned()),
                props: config_props,
                as_const: false,
            },
            Item::Raw(concat(vec![text("export default nextConfig;")])),
        ],
    };

    vec![
        text_file("package.json", package),
        text_file("tsconfig.json", TSCONFIG.to_owned()),
        text_file("eslint.config.mjs", ESLINT_CONFIG.to_owned()),
        text_file("pnpm-workspace.yaml", PNPM_WORKSPACE.to_owned()),
        text_file(".gitignore", GITIGNORE.to_owned()),
        text_file("AGENTS.md", AGENTS.to_owned()),
        text_file("README.md", readme(doc)),
        module_file("next.config.ts".to_owned(), &config),
    ]
}

fn readme(doc: &Document) -> String {
    format!(
        "# {}\n\nSite exporté par [Deep Atelier](https://github.com/CheikhMohamedD/deep_atelier) : Next.js {NEXT_VERSION}, TypeScript strict, Tailwind CSS v4. Le code ne dépend pas de Deep Atelier.\n\n```sh\npnpm install\npnpm dev\n```\n",
        doc.settings.site_name
    )
}

const TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ES2017",
    "lib": ["dom", "dom.iterable", "esnext"],
    "allowJs": true,
    "skipLibCheck": true,
    "strict": true,
    "noEmit": true,
    "esModuleInterop": true,
    "module": "esnext",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "jsx": "react-jsx",
    "incremental": true,
    "plugins": [
      {
        "name": "next"
      }
    ],
    "paths": {
      "@/*": ["./*"]
    }
  },
  "include": [
    "next-env.d.ts",
    "**/*.ts",
    "**/*.tsx",
    ".next/types/**/*.ts",
    ".next/dev/types/**/*.ts",
    "**/*.mts"
  ],
  "exclude": ["node_modules"]
}
"#;

const ESLINT_CONFIG: &str = r#"import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

const eslintConfig = defineConfig([
  ...nextVitals,
  ...nextTs,
  // Override default ignores of eslint-config-next.
  globalIgnores([
    // Default ignores of eslint-config-next:
    ".next/**",
    "out/**",
    "build/**",
    "next-env.d.ts",
  ]),
]);

export default eslintConfig;
"#;

const PNPM_WORKSPACE: &str = "allowBuilds:\n  sharp: false\n  unrs-resolver: false\n";

const GITIGNORE: &str = "# See https://help.github.com/articles/ignoring-files/ for more about ignoring files.

# dependencies
/node_modules
/.pnp
.pnp.*
.yarn/*
!.yarn/patches
!.yarn/plugins
!.yarn/releases
!.yarn/versions

# testing
/coverage

# next.js
/.next/
/out/

# production
/build

# misc
.DS_Store
*.pem

# debug
npm-debug.log*
yarn-debug.log*
yarn-error.log*
.pnpm-debug.log*

# env files (can opt-in for committing if needed)
.env*

# vercel
.vercel

# typescript
*.tsbuildinfo
next-env.d.ts
";

const AGENTS: &str = "<!-- BEGIN:nextjs-agent-rules -->

## This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->
";
