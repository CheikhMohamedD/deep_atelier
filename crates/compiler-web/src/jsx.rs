//! Arbre JSX émis et sa mise en forme, selon les règles JSX de Prettier 3 (`printJsxElement`,
//! `printJsxChildren`, `printJsxOpeningElement`, `printJsxExpressionContainer`).

use ir::NodeId;

use crate::doc::{
    Doc, Mark, concat, conditional_group, fill, group, group_with, hardline, if_break, indent, join, line, softline,
    text, will_break,
};

/// Nœud JSX.
#[derive(Debug, Clone, PartialEq)]
pub enum Jsx {
    Element(Element),
    Fragment(Vec<Jsx>),
    /// Texte JSX déjà échappé, espaces simples ; un espace en tête ou en fin est significatif.
    Text(String),
    /// `{expression}`.
    Expr(Expr),
    /// Code libre (`RawCode`) émis tel quel, ligne par ligne.
    Raw(String),
}

/// Élément JSX ; sans enfant, il est auto-fermant.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<Attr>,
    pub children: Vec<Jsx>,
    /// Nœud de l'IR rendu par l'élément (source map).
    pub node: Option<NodeId>,
}

impl Element {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attrs: Vec::new(),
            children: Vec::new(),
            node: None,
        }
    }

    pub fn attr(mut self, name: impl Into<String>, value: AttrValue) -> Self {
        self.attrs.push(Attr {
            name: name.into(),
            value,
        });
        self
    }

    pub fn string(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attr(name, AttrValue::Str(value.into()))
    }

    pub fn child(mut self, child: Jsx) -> Self {
        self.children.push(child);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub name: String,
    pub value: AttrValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    /// Attribut booléen (`priority`).
    Flag,
    /// Chaîne non échappée.
    Str(String),
    Expr(Expr),
    /// Objet étalé (`{...rest}`) : le nom de l'attribut est celui de l'objet.
    Spread,
}

/// Expression émise.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Identifiant ou expression simple qui ne se coupe pas (`title`, `1200`,
    /// `intentClasses[intent]`).
    Atom(String),
    /// Fonction fléchée ou gabarit de chaîne : imprimée en ligne dans `{…}`.
    Inline(String),
    /// Chaîne littérale (non échappée).
    Str(String),
    /// Appel à arguments simples (`twMerge("…", className)`).
    Call {
        callee: String,
        args: Vec<Expr>,
    },
    /// `condition && <jsx>`.
    And {
        condition: String,
        then: Box<Jsx>,
    },
    Jsx(Box<Jsx>),
}

// ---------------------------------------------------------------- chaînes

/// Chaîne TypeScript à la Prettier : guillemets doubles, sauf s'il y a plus de guillemets
/// doubles que simples dans la valeur.
pub fn string_literal(value: &str) -> String {
    let doubles = value.matches('"').count();
    let singles = value.matches('\'').count();
    let quote = if doubles > singles { '\'' } else { '"' };
    let mut out = String::with_capacity(value.len() + 2);
    out.push(quote);
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Valeur d'attribut JSX à la Prettier : pas d'échappement par barre oblique en JSX, le
/// guillemet choisi est remplacé par son entité.
fn jsx_attribute_string(value: &str) -> String {
    let value = value.replace(['\n', '\r'], " ");
    let doubles = value.matches('"').count();
    let singles = value.matches('\'').count();
    if doubles > singles {
        format!("'{}'", value.replace('\'', "&apos;"))
    } else {
        format!("\"{}\"", value.replace('"', "&quot;"))
    }
}

/// Échappe un texte pour le corps d'un élément JSX (syntaxe et règle ESLint
/// `react/no-unescaped-entities`).
pub fn escape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '{' => out.push_str("&#123;"),
            '}' => out.push_str("&#125;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------- impression

/// Document d'un nœud JSX placé dans un contexte qui l'entoure de parenthèses s'il se coupe
/// (argument de `return`, membre droit de `&&`).
pub fn wrapped(jsx: &Jsx) -> Doc {
    group(concat(vec![
        if_break(text("("), text("")),
        indent(concat(vec![softline(), jsx_doc(jsx)])),
        softline(),
        if_break(text(")"), text("")),
    ]))
}

/// Document d'un nœud JSX sans parenthèses.
pub fn jsx_doc(jsx: &Jsx) -> Doc {
    match jsx {
        Jsx::Element(element) => {
            let doc = element_doc(element);
            match &element.node {
                Some(id) => concat(vec![
                    Doc::Mark(Mark::Start(id.clone())),
                    doc,
                    Doc::Mark(Mark::End(id.clone())),
                ]),
                None => doc,
            }
        }
        Jsx::Fragment(children) => {
            if children.is_empty() {
                return text("<></>");
            }
            element_internal(text("<>"), text("</>"), children, false)
        }
        Jsx::Text(value) => text(value.trim().to_owned()),
        Jsx::Expr(expr) => expression_container(expr, true),
        Jsx::Raw(code) => {
            let lines: Vec<Doc> = code.trim().lines().map(|l| text(l.trim_end().to_owned())).collect();
            join(&hardline(), lines)
        }
    }
}

fn element_doc(element: &Element) -> Doc {
    let opening = opening_doc(element);
    if element.children.is_empty() {
        return opening;
    }
    let closing = text(format!("</{}>", element.name));
    element_internal(opening, closing, &element.children, element.attrs.len() > 1)
}

fn opening_doc(element: &Element) -> Doc {
    let self_closing = element.children.is_empty();
    let name = &element.name;
    if self_closing && element.attrs.is_empty() {
        return text(format!("<{name} />"));
    }
    if !self_closing && element.attrs.is_empty() {
        return text(format!("<{name}>"));
    }
    // Un seul attribut chaîne : la balise ouvrante ne se coupe pas.
    if let [attr] = element.attrs.as_slice()
        && let AttrValue::Str(value) = &attr.value
        && !value.contains('\n')
    {
        return group(concat(vec![
            text(format!("<{name} ")),
            attr_doc(attr),
            text(if self_closing { " />" } else { ">" }),
        ]));
    }
    let attrs: Vec<Doc> = element
        .attrs
        .iter()
        .map(|a| concat(vec![line(), attr_doc(a)]))
        .collect();
    let end = if self_closing {
        concat(vec![line(), text("/>")])
    } else {
        concat(vec![softline(), text(">")])
    };
    group(concat(vec![text(format!("<{name}")), indent(concat(attrs)), end]))
}

fn attr_doc(attr: &Attr) -> Doc {
    match &attr.value {
        AttrValue::Flag => text(attr.name.clone()),
        AttrValue::Spread => text(format!("{{...{}}}", attr.name)),
        AttrValue::Str(value) => text(format!("{}={}", attr.name, jsx_attribute_string(value))),
        AttrValue::Expr(expr) => concat(vec![text(format!("{}=", attr.name)), expression_container(expr, false)]),
    }
}

/// `{expression}` : en ligne pour les appels, fonctions fléchées, gabarits, et pour un `&&`
/// enfant d'un élément ; sinon avec un saut possible après `{`.
fn expression_container(expr: &Expr, in_children: bool) -> Doc {
    let inline = match expr {
        Expr::Call { .. } | Expr::Inline(_) => true,
        Expr::And { .. } => in_children,
        Expr::Atom(_) | Expr::Str(_) | Expr::Jsx(_) => false,
    };
    let printed = expression_doc(expr);
    if inline {
        group(concat(vec![text("{"), printed, text("}")]))
    } else {
        group(concat(vec![
            text("{"),
            indent(concat(vec![softline(), printed])),
            softline(),
            text("}"),
        ]))
    }
}

/// Document d'une expression.
pub fn expression_doc(expr: &Expr) -> Doc {
    match expr {
        Expr::Atom(value) | Expr::Inline(value) => text(value.clone()),
        Expr::Str(value) => text(string_literal(value)),
        Expr::Call { callee, args } => {
            if args.is_empty() {
                return text(format!("{callee}()"));
            }
            let separator = concat(vec![text(","), line()]);
            group(concat(vec![
                text(format!("{callee}(")),
                indent(concat(vec![
                    softline(),
                    join(&separator, args.iter().map(expression_doc).collect()),
                ])),
                if_break(text(","), text("")),
                softline(),
                text(")"),
            ]))
        }
        Expr::And { condition, then } => concat(vec![text(format!("{condition} && ")), wrapped(then)]),
        Expr::Jsx(jsx) => jsx_doc(jsx),
    }
}

/// Élément de la liste des enfants préparée par `printJsxChildren`.
#[derive(Debug, Clone, PartialEq)]
enum Part {
    /// Mot de texte, ou chaîne vide (marqueur de Prettier).
    Word(String),
    Child(Doc),
    /// `jsxWhitespace` : espace à plat, `{" "}` puis saut de ligne coupé.
    JsxSpace,
    Soft,
    Hard,
    /// Séparateur entre deux mots.
    Line,
    /// `{" "}` littéral.
    RawSpace,
    /// `{" "}` suivi d'un saut de ligne.
    RawSpaceLine,
}

fn is_empty_word(part: Option<&Part>) -> bool {
    matches!(part, Some(Part::Word(w)) if w.is_empty())
}

const JSX_SPACE: &str = "{\" \"}";

impl Part {
    fn doc(&self) -> Doc {
        match self {
            Part::Word(word) => text(word.clone()),
            Part::Child(doc) => doc.clone(),
            Part::JsxSpace => if_break(concat(vec![text(JSX_SPACE), softline()]), text(" ")),
            Part::Soft => softline(),
            Part::Hard => hardline(),
            Part::Line => line(),
            Part::RawSpace => text(JSX_SPACE),
            Part::RawSpaceLine => concat(vec![text(JSX_SPACE), hardline()]),
        }
    }

    fn is_line(&self) -> bool {
        matches!(self, Part::Soft | Part::Hard | Part::Line)
    }
}

/// Texte significatif : contient autre chose que des espaces, ou des espaces sans saut de ligne.
fn is_meaningful_text(value: &str) -> bool {
    value.chars().any(|c| !matches!(c, ' ' | '\n' | '\r' | '\t')) || !value.contains('\n')
}

/// Découpe un texte en mots et séparateurs d'espaces (`split(/([ \n\r\t]+)/)`).
fn split_words(value: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut in_space = false;
    for c in value.chars() {
        let space = matches!(c, ' ' | '\n' | '\r' | '\t');
        if space != in_space {
            out.push(String::new());
            in_space = space;
        }
        out.last_mut().expect("non vide").push(c);
    }
    if in_space {
        out.push(String::new());
    }
    out
}

fn is_self_closing(jsx: &Jsx) -> bool {
    matches!(jsx, Jsx::Element(e) if e.children.is_empty())
}

/// `separatorNoWhitespace` : un élément auto-fermant collé à un texte force un saut de ligne,
/// sauf devant un mot d'un seul caractère.
fn separator_no_whitespace(word: &str, child: &Jsx, next: Option<&Jsx>) -> Part {
    if is_self_closing(child) || next.is_some_and(is_self_closing) {
        if word.chars().count() == 1 {
            Part::Soft
        } else {
            Part::Hard
        }
    } else {
        Part::Soft
    }
}

/// `printJsxChildren`.
fn children_parts(children: &[Jsx]) -> Vec<Part> {
    let mut parts = Vec::new();
    for (i, child) in children.iter().enumerate() {
        let next = children.get(i + 1);
        match child {
            Jsx::Text(value) => {
                if is_meaningful_text(value) {
                    let mut words = split_words(value);
                    if words[0].is_empty() {
                        parts.push(Part::Word(String::new()));
                        words.remove(0);
                        // Nos textes n'ont pas de saut de ligne : l'espace de tête est `jsxWhitespace`.
                        parts.push(Part::JsxSpace);
                        words.remove(0);
                    }
                    let mut end_space = None;
                    if words.last().is_some_and(String::is_empty) {
                        words.pop();
                        end_space = words.pop();
                    }
                    if words.is_empty() {
                        continue;
                    }
                    for (k, word) in words.iter().enumerate() {
                        parts.push(if k % 2 == 1 {
                            Part::Line
                        } else {
                            Part::Word(word.clone())
                        });
                    }
                    match end_space {
                        Some(_) => parts.push(Part::JsxSpace),
                        None => {
                            let last = words.last().map(String::as_str).unwrap_or_default();
                            parts.push(separator_no_whitespace(last, child, next));
                        }
                    }
                } else if value.contains('\n') {
                    if value.matches('\n').count() > 1 {
                        parts.push(Part::Word(String::new()));
                        parts.push(Part::Hard);
                    }
                } else {
                    parts.push(Part::Word(String::new()));
                    parts.push(Part::JsxSpace);
                }
            }
            _ => {
                parts.push(Part::Child(jsx_doc(child)));
                match next {
                    Some(Jsx::Text(value)) if is_meaningful_text(value) => {
                        let trimmed = value.trim_matches([' ', '\n', '\r', '\t']);
                        let first = split_words(trimmed).into_iter().next().unwrap_or_default();
                        parts.push(separator_no_whitespace(&first, child, next));
                    }
                    _ => parts.push(Part::Hard),
                }
            }
        }
    }
    parts
}

/// `printJsxElementInternal`.
fn element_internal(opening: Doc, closing: Doc, children: &[Jsx], multiple_attrs: bool) -> Doc {
    let contains_tag = children
        .iter()
        .any(|c| matches!(c, Jsx::Element(_) | Jsx::Fragment(_) | Jsx::Raw(_)));
    let contains_multiple_expressions = children.iter().filter(|c| matches!(c, Jsx::Expr(_))).count() > 1;
    let mut forced_break = will_break(&opening) || contains_tag || multiple_attrs || contains_multiple_expressions;
    let contains_text = children
        .iter()
        .any(|c| matches!(c, Jsx::Text(value) if is_meaningful_text(value)));

    let mut parts = children_parts(children);

    // Nettoyage des séparateurs voisins (même ordre et mêmes règles que Prettier).
    let mut i = parts.len() as isize - 2;
    while i >= 0 {
        let k = i as usize;
        let a = parts.get(k);
        let b = parts.get(k + 1);
        let c = parts.get(k + 2);
        let pair_of_empty = is_empty_word(a) && is_empty_word(b);
        let pair_of_hardlines = a == Some(&Part::Hard) && is_empty_word(b) && c == Some(&Part::Hard);
        let line_then_space =
            matches!(a, Some(Part::Hard | Part::Soft)) && is_empty_word(b) && c == Some(&Part::JsxSpace);
        let space_then_line =
            a == Some(&Part::JsxSpace) && is_empty_word(b) && matches!(c, Some(Part::Hard | Part::Soft));
        let double_space = a == Some(&Part::JsxSpace) && is_empty_word(b) && c == Some(&Part::JsxSpace);
        let soft_and_hard = (a == Some(&Part::Soft) && is_empty_word(b) && c == Some(&Part::Hard))
            || (a == Some(&Part::Hard) && is_empty_word(b) && c == Some(&Part::Soft));
        if (pair_of_hardlines && contains_text) || pair_of_empty || line_then_space || double_space {
            parts.drain(k..k + 2);
        } else if space_then_line || soft_and_hard {
            parts.drain(k + 1..(k + 3).min(parts.len()));
        }
        i -= 1;
    }
    // Sauts de ligne et chaînes vides retirés en fin, puis par paires en tête.
    while parts
        .last()
        .is_some_and(|p| p.is_line() || matches!(p, Part::Word(w) if w.is_empty()))
    {
        parts.pop();
    }
    while parts.len() > 1
        && (parts[0].is_line() || is_empty_word(parts.first()))
        && (parts[1].is_line() || is_empty_word(parts.get(1)))
    {
        parts.drain(0..2);
    }

    let mut multiline: Vec<Doc> = Vec::with_capacity(parts.len());
    for (k, part) in parts.iter().enumerate() {
        if *part == Part::JsxSpace {
            if k == 1 && is_empty_word(parts.first()) {
                if parts.len() == 2 {
                    multiline.push(Part::RawSpace.doc());
                    continue;
                }
                multiline.push(Part::RawSpaceLine.doc());
                continue;
            } else if k == parts.len() - 1
                || (k >= 2 && is_empty_word(parts.get(k - 1)) && parts.get(k - 2) == Some(&Part::Hard))
            {
                multiline.push(Part::RawSpace.doc());
                continue;
            }
        }
        let doc = part.doc();
        if will_break(&doc) {
            forced_break = true;
        }
        multiline.push(doc);
    }

    let content = if contains_text {
        fill(multiline)
    } else {
        group_with(concat(multiline), true)
    };
    let multi_line_element = group(concat(vec![
        opening.clone(),
        indent(concat(vec![hardline(), content])),
        hardline(),
        closing.clone(),
    ]));
    if forced_break {
        return multi_line_element;
    }
    let mut flat = vec![opening];
    flat.extend(parts.iter().map(Part::doc));
    flat.push(closing);
    conditional_group(vec![group(concat(flat)), multi_line_element])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::print;

    fn el(name: &str) -> Element {
        Element::new(name)
    }

    fn render(jsx: Jsx) -> String {
        print(wrapped(&jsx)).text
    }

    #[test]
    fn short_elements_stay_on_one_line() {
        let p = el("p").string("className", "text-lg").child(Jsx::Text("Hello".into()));
        assert_eq!(render(Jsx::Element(p)), "<p className=\"text-lg\">Hello</p>");
    }

    #[test]
    fn several_attributes_force_the_children_on_their_own_line() {
        let a = el("a")
            .string("href", "/")
            .string("className", "text-sm")
            .child(Jsx::Text("Home".into()));
        assert_eq!(
            render(Jsx::Element(a)),
            "(\n  <a href=\"/\" className=\"text-sm\">\n    Home\n  </a>\n)"
        );
    }

    #[test]
    fn strings_choose_their_quotes() {
        assert_eq!(string_literal("a \"b\""), "'a \"b\"'");
        assert_eq!(string_literal("l'outil"), "\"l'outil\"");
        assert_eq!(jsx_attribute_string("Le \"meilleur\" outil"), "'Le \"meilleur\" outil'");
        assert_eq!(escape_text("L'outil {x} & co"), "L&apos;outil &#123;x&#125; &amp; co");
    }
}
