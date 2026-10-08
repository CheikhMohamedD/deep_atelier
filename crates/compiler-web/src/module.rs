//! Modules TypeScript émis (imports, types, constantes, fonctions) et leur mise en forme, aux
//! formes que Prettier 3 produit pour ces constructions.

use std::collections::{BTreeMap, BTreeSet};

use crate::doc::{Doc, Printed, concat, group, hardline, if_break, indent, join, line, print, softline, text};
use crate::jsx::{Jsx, string_literal, wrapped};

/// Imports d'un module, regroupés par module source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Imports {
    default: BTreeMap<String, String>,
    named: BTreeMap<String, BTreeSet<String>>,
    types: BTreeMap<String, BTreeSet<String>>,
    side_effects: BTreeSet<String>,
}

impl Imports {
    pub fn default_import(&mut self, module: &str, name: &str) {
        self.default.insert(module.to_owned(), name.to_owned());
    }

    pub fn named(&mut self, module: &str, name: &str) {
        self.named.entry(module.to_owned()).or_default().insert(name.to_owned());
    }

    pub fn types(&mut self, module: &str, name: &str) {
        self.types.entry(module.to_owned()).or_default().insert(name.to_owned());
    }

    pub fn side_effect(&mut self, module: &str) {
        self.side_effects.insert(module.to_owned());
    }

    pub fn is_empty(&self) -> bool {
        self.default.is_empty() && self.named.is_empty() && self.types.is_empty() && self.side_effects.is_empty()
    }

    /// Vrai si le module importe quelque chose de `module`.
    pub fn has_module(&self, module: &str) -> bool {
        self.default.contains_key(module)
            || self.named.contains_key(module)
            || self.types.contains_key(module)
            || self.side_effects.contains(module)
    }

    /// Rang d'un module : Next, React, paquets tiers, puis modules du projet.
    fn rank(module: &str) -> (u8, String) {
        let rank = if module == "next" || module.starts_with("next/") {
            0
        } else if module == "react" {
            1
        } else if module.starts_with("@/") {
            3
        } else if module.starts_with("./") {
            4
        } else {
            2
        };
        (rank, module.to_owned())
    }

    fn docs(&self) -> Vec<Doc> {
        let mut modules: BTreeSet<(u8, String)> = BTreeSet::new();
        for module in self
            .default
            .keys()
            .chain(self.named.keys())
            .chain(self.types.keys())
            .chain(self.side_effects.iter())
        {
            modules.insert(Self::rank(module));
        }
        let mut out = Vec::new();
        for (_, module) in modules {
            let from = string_literal(&module);
            if let Some(types) = self.types.get(&module) {
                out.push(specifiers("import type ", types, &from));
            }
            let named = self.named.get(&module);
            match (self.default.get(&module), named) {
                (Some(name), Some(names)) => {
                    out.push(specifiers(&format!("import {name}, "), names, &from));
                }
                (Some(name), None) => out.push(text(format!("import {name} from {from};"))),
                (None, Some(names)) => out.push(specifiers("import ", names, &from)),
                (None, None) => {}
            }
            if self.side_effects.contains(&module) {
                out.push(text(format!("import {from};")));
            }
        }
        out
    }
}

/// `import { A, B } from "m";`, coupé un nom par ligne s'il ne tient pas.
fn specifiers(head: &str, names: &BTreeSet<String>, from: &str) -> Doc {
    let separator = concat(vec![text(","), line()]);
    group(concat(vec![
        text(format!("{head}{{")),
        indent(concat(vec![
            line(),
            join(&separator, names.iter().map(|n| text(n.clone())).collect()),
        ])),
        if_break(text(","), text("")),
        line(),
        text(format!("}} from {from};")),
    ]))
}

/// Valeur d'une propriété d'objet.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    /// Expression imprimée telle quelle.
    Raw(String),
    /// Objet à plat s'il tient, sinon déployé.
    Object(Vec<(String, Value)>),
    /// Objet toujours déployé (saut de ligne après `{` dans la source, que Prettier conserve).
    Expanded(Vec<(String, Value)>),
    Array(Vec<Value>),
}

/// Clé d'objet : identifiant tel quel, sinon chaîne.
fn key_text(key: &str) -> String {
    let identifier = key
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if identifier {
        key.to_owned()
    } else {
        string_literal(key)
    }
}

fn value_doc(value: &Value) -> Doc {
    match value {
        Value::Str(s) => text(string_literal(s)),
        Value::Raw(s) => text(s.clone()),
        Value::Object(props) => {
            let separator = concat(vec![text(","), line()]);
            group(concat(vec![
                text("{"),
                indent(concat(vec![
                    line(),
                    join(&separator, props.iter().map(|(k, v)| property_doc(k, v)).collect()),
                ])),
                if_break(text(","), text("")),
                line(),
                text("}"),
            ]))
        }
        Value::Expanded(props) => expanded_object(props),
        Value::Array(items) => {
            // Plusieurs objets d'au moins deux propriétés : un par ligne (règle de Prettier).
            let concise = items.len() > 1
                && items.iter().all(|item| match item {
                    Value::Object(props) | Value::Expanded(props) => props.len() > 1,
                    Value::Array(inner) => inner.len() > 1,
                    _ => false,
                });
            let separator = concat(vec![text(","), line()]);
            crate::doc::group_with(
                concat(vec![
                    text("["),
                    indent(concat(vec![
                        softline(),
                        join(&separator, items.iter().map(value_doc).collect()),
                    ])),
                    if_break(text(","), text("")),
                    softline(),
                    text("]"),
                ]),
                concise,
            )
        }
    }
}

/// `clé: valeur` ; une chaîne trop longue passe à la ligne après `:` sauf si la clé est courte
/// (moins de `tabWidth + 3` colonnes), comme la mise en page « break-after-operator ».
fn property_doc(key: &str, value: &Value) -> Doc {
    let key = key_text(key);
    match value {
        Value::Str(_) if key.chars().count() >= 5 => group(concat(vec![
            text(format!("{key}:")),
            group(indent(concat(vec![line(), value_doc(value)]))),
        ])),
        _ => concat(vec![text(format!("{key}: ")), value_doc(value)]),
    }
}

/// Objet littéral déployé (une propriété par ligne), comme Prettier le conserve quand la source
/// a un saut de ligne après `{`.
fn expanded_object(props: &[(String, Value)]) -> Doc {
    let mut parts = vec![text("{")];
    let mut inner = Vec::new();
    for (key, value) in props {
        inner.push(hardline());
        inner.push(property_doc(key, value));
        inner.push(text(","));
    }
    parts.push(indent(concat(inner)));
    parts.push(hardline());
    parts.push(text("}"));
    concat(parts)
}

/// Membre d'un type objet.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub name: String,
    pub optional: bool,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Raw(String),
    /// Union de littéraux de chaîne.
    Union(Vec<String>),
}

fn type_doc(ty: &TypeExpr) -> Doc {
    match ty {
        TypeExpr::Raw(s) => text(s.clone()),
        TypeExpr::Union(values) => {
            let separator = concat(vec![line(), text("| ")]);
            group(indent(concat(vec![
                if_break(concat(vec![line(), text("| ")]), text("")),
                join(&separator, values.iter().map(|v| text(string_literal(v))).collect()),
            ])))
        }
    }
}

/// Paramètre déstructuré d'une fonction composant.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    /// Valeur par défaut (expression déjà imprimée).
    pub default: Option<String>,
    /// Reste des props (`...rest`), toujours en dernier.
    pub rest: bool,
}

impl Param {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default: None,
            rest: false,
        }
    }

    pub fn with_default(name: impl Into<String>, default: String) -> Self {
        Self {
            name: name.into(),
            default: Some(default),
            rest: false,
        }
    }

    pub fn rest(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default: None,
            rest: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Instruction d'une ligne (`const [open, setOpen] = useState(false);`).
    Raw(String),
    /// `if (!condition) { return null; }`.
    ReturnNullUnless(String),
    Return(Jsx),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Export {
    None,
    Named,
    Default,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    TypeAlias {
        name: String,
        /// Type intersecté avant l'objet (`AriaAttributes & { … }`).
        intersection: Option<String>,
        members: Vec<Member>,
    },
    Const {
        export: bool,
        name: String,
        annotation: Option<String>,
        props: Vec<(String, Value)>,
        as_const: bool,
    },
    Function {
        export: Export,
        name: String,
        params: Option<(Vec<Param>, String)>,
        body: Vec<Statement>,
    },
    /// Déclaration d'une ligne (`const inter = Inter({ … });` déjà imprimée).
    Raw(Doc),
}

fn item_doc(item: &Item) -> Doc {
    match item {
        Item::TypeAlias {
            name,
            intersection,
            members,
        } => {
            let head = match intersection {
                Some(base) if members.is_empty() => return text(format!("type {name} = {base};")),
                Some(base) => format!("type {name} = {base} & {{"),
                None => format!("type {name} = {{"),
            };
            let mut inner = Vec::new();
            for member in members {
                inner.push(hardline());
                inner.push(concat(vec![
                    text(format!("{}{}: ", member.name, if member.optional { "?" } else { "" })),
                    type_doc(&member.ty),
                    text(";"),
                ]));
            }
            concat(vec![text(head), indent(concat(inner)), hardline(), text("};")])
        }
        Item::Const {
            export,
            name,
            annotation,
            props,
            as_const,
        } => {
            let head = format!(
                "{}const {name}{} = ",
                if *export { "export " } else { "" },
                annotation.as_ref().map(|a| format!(": {a}")).unwrap_or_default()
            );
            concat(vec![
                text(head),
                expanded_object(props),
                text(if *as_const { " as const;" } else { ";" }),
            ])
        }
        Item::Function {
            export,
            name,
            params,
            body,
        } => {
            let head = match export {
                Export::None => format!("function {name}("),
                Export::Named => format!("export function {name}("),
                Export::Default => format!("export default function {name}("),
            };
            let params_doc = match params {
                None => text(""),
                Some((params, annotation)) => {
                    let separator = concat(vec![text(","), line()]);
                    let props: Vec<Doc> = params
                        .iter()
                        .map(|p| match (&p.default, p.rest) {
                            (_, true) => text(format!("...{}", p.name)),
                            (Some(default), false) => text(format!("{} = {default}", p.name)),
                            (None, false) => text(p.name.clone()),
                        })
                        .collect();
                    // Pas de virgule finale après un reste (erreur de syntaxe).
                    let trailing = if params.last().is_some_and(|p| p.rest) {
                        text("")
                    } else {
                        if_break(text(","), text(""))
                    };
                    concat(vec![
                        group(concat(vec![
                            text("{"),
                            indent(concat(vec![line(), join(&separator, props)])),
                            trailing,
                            line(),
                            text("}"),
                        ])),
                        text(format!(": {annotation}")),
                    ])
                }
            };
            let mut inner = Vec::new();
            for statement in body {
                inner.push(hardline());
                inner.push(match statement {
                    Statement::Raw(s) => text(s.clone()),
                    Statement::ReturnNullUnless(condition) => concat(vec![
                        text(format!("if (!{condition}) {{")),
                        indent(concat(vec![hardline(), text("return null;")])),
                        hardline(),
                        text("}"),
                    ]),
                    Statement::Return(jsx) => concat(vec![text("return "), wrapped(jsx), text(";")]),
                });
            }
            concat(vec![
                text(head),
                params_doc,
                text(") {"),
                indent(concat(inner)),
                hardline(),
                text("}"),
            ])
        }
        Item::Raw(doc) => doc.clone(),
    }
}

/// Module TypeScript complet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Module {
    pub client: bool,
    pub imports: Imports,
    pub items: Vec<Item>,
}

impl Module {
    /// Document du module : directive, imports, puis déclarations séparées d'une ligne vide.
    pub fn doc(&self) -> Doc {
        let mut blocks: Vec<Doc> = Vec::new();
        if self.client {
            blocks.push(text("\"use client\";"));
        }
        let imports = self.imports.docs();
        if !imports.is_empty() {
            blocks.push(join(&hardline(), imports));
        }
        for item in &self.items {
            blocks.push(item_doc(item));
        }
        concat(vec![join(&concat(vec![hardline(), hardline()]), blocks), hardline()])
    }

    pub fn print(&self) -> Printed {
        print(self.doc())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_break_after_the_colon_unless_the_key_is_short() {
        let long = "x".repeat(90);
        let module = Module {
            items: vec![Item::Const {
                export: false,
                name: "sizes".into(),
                annotation: None,
                props: vec![
                    ("sm".into(), Value::Str(long.clone())),
                    ("medium".into(), Value::Str(long.clone())),
                    ("two words".into(), Value::Str("x".into())),
                ],
                as_const: true,
            }],
            ..Module::default()
        };
        let printed = module.print().text;
        assert_eq!(
            printed,
            format!(
                "const sizes = {{\n  sm: \"{long}\",\n  medium:\n    \"{long}\",\n  \"two words\": \"x\",\n}} as const;\n"
            )
        );
    }

    #[test]
    fn long_import_lists_break() {
        let mut imports = Imports::default();
        for name in [
            "ArrowRightIcon",
            "MenuIcon",
            "XIcon",
            "CheckIcon",
            "StarIcon",
            "ZapIcon",
            "ShieldIcon",
        ] {
            imports.named("lucide-react", name);
        }
        imports.default_import("next/link", "Link");
        let module = Module {
            imports,
            ..Module::default()
        };
        assert_eq!(
            module.print().text,
            "import Link from \"next/link\";\nimport {\n  ArrowRightIcon,\n  CheckIcon,\n  MenuIcon,\n  ShieldIcon,\n  StarIcon,\n  XIcon,\n  ZapIcon,\n} from \"lucide-react\";\n"
        );
    }
}
