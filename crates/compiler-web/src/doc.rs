//! Documents « à la Prettier » et leur impression.
//!
//! Le compilateur décrit la forme du code (groupes, retraits, sauts de ligne possibles,
//! remplissage) ; l'imprimeur choisit les coupures avec l'algorithme de `printDocToString` de
//! Prettier 3, pour une sortie identique à celle de Prettier sur le code que nous émettons.
//! Des marques sans largeur relèvent la position des nœuds (source map).

use ir::NodeId;
use unicode_width::UnicodeWidthStr;

/// Largeur de ligne (option `printWidth` de Prettier).
pub const PRINT_WIDTH: usize = 80;
/// Largeur d'un niveau de retrait (option `tabWidth` de Prettier).
const TAB_WIDTH: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum Doc {
    /// Texte sans saut de ligne.
    Text(String),
    Concat(Vec<Doc>),
    Group(Box<Group>),
    Indent(Box<Doc>),
    Line(Line),
    /// Contenu en mode coupé, contenu en mode à plat.
    IfBreak(Box<Doc>, Box<Doc>),
    /// Alternance contenu / séparateur, remplie ligne par ligne.
    Fill(Vec<Doc>),
    /// Force la coupure des groupes englobants.
    BreakParent,
    Mark(Mark),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub contents: Doc,
    pub should_break: bool,
    /// États essayés dans l'ordre (`conditionalGroup`) ; vide pour un groupe simple.
    pub expanded_states: Vec<Doc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// Espace à plat, saut de ligne coupé.
    Normal,
    /// Rien à plat, saut de ligne coupé.
    Soft,
    /// Toujours un saut de ligne.
    Hard,
}

/// Début ou fin de l'élément qui rend un nœud.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    Start(NodeId),
    End(NodeId),
}

// ---------------------------------------------------------------- constructeurs

pub fn text(value: impl Into<String>) -> Doc {
    Doc::Text(value.into())
}

pub fn concat(parts: Vec<Doc>) -> Doc {
    Doc::Concat(parts)
}

pub fn group(contents: Doc) -> Doc {
    group_with(contents, false)
}

pub fn group_with(contents: Doc, should_break: bool) -> Doc {
    Doc::Group(Box::new(Group {
        contents,
        should_break,
        expanded_states: Vec::new(),
    }))
}

/// `conditionalGroup` : le premier état à plat s'il tient, sinon le premier état suivant qui
/// tient, sinon le dernier, coupé.
pub fn conditional_group(states: Vec<Doc>) -> Doc {
    Doc::Group(Box::new(Group {
        contents: states[0].clone(),
        should_break: false,
        expanded_states: states,
    }))
}

pub fn indent(contents: Doc) -> Doc {
    Doc::Indent(Box::new(contents))
}

pub fn line() -> Doc {
    Doc::Line(Line::Normal)
}

pub fn softline() -> Doc {
    Doc::Line(Line::Soft)
}

pub fn hardline() -> Doc {
    Doc::Concat(vec![Doc::Line(Line::Hard), Doc::BreakParent])
}

pub fn if_break(broken: Doc, flat: Doc) -> Doc {
    Doc::IfBreak(Box::new(broken), Box::new(flat))
}

pub fn fill(parts: Vec<Doc>) -> Doc {
    Doc::Fill(parts)
}

pub fn join(separator: &Doc, docs: Vec<Doc>) -> Doc {
    let mut parts = Vec::with_capacity(docs.len() * 2);
    for (i, doc) in docs.into_iter().enumerate() {
        if i > 0 {
            parts.push(separator.clone());
        }
        parts.push(doc);
    }
    Doc::Concat(parts)
}

/// `willBreak` : le document contient un groupe coupé, un saut de ligne forcé ou un
/// `breakParent` (sans explorer les états d'un `conditionalGroup`).
pub fn will_break(doc: &Doc) -> bool {
    match doc {
        Doc::Group(g) => g.should_break || will_break(&g.contents),
        Doc::Line(Line::Hard) | Doc::BreakParent => true,
        Doc::Concat(parts) | Doc::Fill(parts) => parts.iter().any(will_break),
        Doc::Indent(contents) => will_break(contents),
        Doc::IfBreak(broken, flat) => will_break(broken) || will_break(flat),
        Doc::Text(_) | Doc::Line(_) | Doc::Mark(_) => false,
    }
}

/// `propagateBreaks` : un groupe qui contient une coupure forcée est coupé, et le fait savoir à
/// ses parents ; un `conditionalGroup` n'est pas coupé par ses états.
fn propagate_breaks(doc: &mut Doc) -> bool {
    match doc {
        Doc::BreakParent => true,
        Doc::Concat(parts) | Doc::Fill(parts) => {
            let mut any = false;
            for part in parts {
                any |= propagate_breaks(part);
            }
            any
        }
        Doc::Indent(contents) => propagate_breaks(contents),
        Doc::IfBreak(broken, flat) => {
            let a = propagate_breaks(broken);
            let b = propagate_breaks(flat);
            a || b
        }
        Doc::Group(g) => {
            if g.expanded_states.is_empty() {
                if propagate_breaks(&mut g.contents) {
                    g.should_break = true;
                }
                g.should_break
            } else {
                for state in &mut g.expanded_states {
                    propagate_breaks(state);
                }
                propagate_breaks(&mut g.contents);
                false
            }
        }
        Doc::Text(_) | Doc::Line(_) | Doc::Mark(_) => false,
    }
}

// ---------------------------------------------------------------- impression

/// Texte imprimé et position (en octets) de chaque marque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Printed {
    pub text: String,
    pub marks: Vec<(Mark, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Break,
    Flat,
}

#[derive(Debug, Clone, Copy)]
enum Item<'a> {
    Doc(&'a Doc),
    Concat(&'a [Doc]),
    Fill(&'a [Doc]),
}

#[derive(Debug, Clone, Copy)]
struct Cmd<'a> {
    indent: usize,
    mode: Mode,
    item: Item<'a>,
}

fn width_of(text: &str) -> usize {
    if text.is_ascii() {
        text.len()
    } else {
        UnicodeWidthStr::width(text)
    }
}

/// Imprime un document à la largeur [`PRINT_WIDTH`].
pub fn print(mut doc: Doc) -> Printed {
    propagate_breaks(&mut doc);
    let width = PRINT_WIDTH as isize;
    let mut out = String::new();
    let mut marks = Vec::new();
    let mut pos: isize = 0;
    let mut should_remeasure = false;
    let mut cmds: Vec<Cmd<'_>> = vec![Cmd {
        indent: 0,
        mode: Mode::Break,
        item: Item::Doc(&doc),
    }];
    while let Some(Cmd { indent, mode, item }) = cmds.pop() {
        let doc = match item {
            Item::Concat(parts) => {
                for part in parts.iter().rev() {
                    cmds.push(Cmd {
                        indent,
                        mode,
                        item: Item::Doc(part),
                    });
                }
                continue;
            }
            Item::Fill(parts) => {
                print_fill(&mut cmds, indent, mode, parts, width - pos);
                continue;
            }
            Item::Doc(doc) => doc,
        };
        match doc {
            Doc::Text(value) => {
                out.push_str(value);
                pos += width_of(value) as isize;
            }
            Doc::Concat(parts) => {
                for part in parts.iter().rev() {
                    cmds.push(Cmd {
                        indent,
                        mode,
                        item: Item::Doc(part),
                    });
                }
            }
            Doc::Indent(contents) => cmds.push(Cmd {
                indent: indent + 1,
                mode,
                item: Item::Doc(contents),
            }),
            Doc::Group(g) => {
                let flat_ok = mode == Mode::Flat && !should_remeasure;
                if flat_ok {
                    cmds.push(Cmd {
                        indent,
                        mode: if g.should_break { Mode::Break } else { Mode::Flat },
                        item: Item::Doc(&g.contents),
                    });
                    continue;
                }
                should_remeasure = false;
                let next = Cmd {
                    indent,
                    mode: Mode::Flat,
                    item: Item::Doc(&g.contents),
                };
                let rem = width - pos;
                if !g.should_break && fits(next, &cmds, rem, false) {
                    cmds.push(next);
                } else if g.expanded_states.is_empty() {
                    cmds.push(Cmd {
                        indent,
                        mode: Mode::Break,
                        item: Item::Doc(&g.contents),
                    });
                } else {
                    let most_expanded = &g.expanded_states[g.expanded_states.len() - 1];
                    if g.should_break {
                        cmds.push(Cmd {
                            indent,
                            mode: Mode::Break,
                            item: Item::Doc(most_expanded),
                        });
                    } else {
                        let mut chosen = None;
                        for state in &g.expanded_states[1..] {
                            let cmd = Cmd {
                                indent,
                                mode: Mode::Flat,
                                item: Item::Doc(state),
                            };
                            if fits(cmd, &cmds, rem, false) {
                                chosen = Some(cmd);
                                break;
                            }
                        }
                        cmds.push(chosen.unwrap_or(Cmd {
                            indent,
                            mode: Mode::Break,
                            item: Item::Doc(most_expanded),
                        }));
                    }
                }
            }
            Doc::IfBreak(broken, flat) => cmds.push(Cmd {
                indent,
                mode,
                item: Item::Doc(if mode == Mode::Break { broken } else { flat }),
            }),
            Doc::Line(kind) => {
                if mode == Mode::Flat && *kind != Line::Hard {
                    if *kind == Line::Normal {
                        out.push(' ');
                        pos += 1;
                    }
                    continue;
                }
                if mode == Mode::Flat {
                    should_remeasure = true;
                }
                // Espaces de fin de ligne retirés, comme `trim` de Prettier.
                let trimmed = out.trim_end_matches([' ', '\t']).len();
                out.truncate(trimmed);
                out.push('\n');
                for _ in 0..indent * TAB_WIDTH {
                    out.push(' ');
                }
                pos = (indent * TAB_WIDTH) as isize;
            }
            Doc::Fill(parts) => print_fill(&mut cmds, indent, mode, parts, width - pos),
            Doc::BreakParent => {}
            Doc::Mark(mark) => marks.push((mark.clone(), out.len())),
        }
    }
    let len = out.len();
    for (_, offset) in &mut marks {
        *offset = (*offset).min(len);
    }
    Printed { text: out, marks }
}

/// Remplissage : chaque contenu à plat s'il tient, séparateur coupé sinon (algorithme `fill`).
fn print_fill<'a>(cmds: &mut Vec<Cmd<'a>>, indent: usize, mode: Mode, parts: &'a [Doc], rem: isize) {
    if parts.is_empty() {
        return;
    }
    let content = Item::Doc(&parts[0]);
    let flat = |item| Cmd {
        indent,
        mode: Mode::Flat,
        item,
    };
    let broken = |item| Cmd {
        indent,
        mode: Mode::Break,
        item,
    };
    let content_fits = fits(flat(content), &[], rem, true);
    if parts.len() == 1 {
        cmds.push(if content_fits { flat(content) } else { broken(content) });
        return;
    }
    let whitespace = Item::Doc(&parts[1]);
    if parts.len() == 2 {
        if content_fits {
            cmds.push(flat(whitespace));
            cmds.push(flat(content));
        } else {
            cmds.push(broken(whitespace));
            cmds.push(broken(content));
        }
        return;
    }
    let remaining = Cmd {
        indent,
        mode,
        item: Item::Fill(&parts[2..]),
    };
    let first_and_second_fit = fits(flat(Item::Concat(&parts[0..3])), &[], rem, true);
    cmds.push(remaining);
    if first_and_second_fit {
        cmds.push(flat(whitespace));
        cmds.push(flat(content));
    } else if content_fits {
        cmds.push(broken(whitespace));
        cmds.push(flat(content));
    } else {
        cmds.push(broken(whitespace));
        cmds.push(broken(content));
    }
}

/// Vrai si `next` tient dans `width` colonnes jusqu'au prochain saut de ligne, en comptant la
/// suite de la ligne (`rest`) ; `must_be_flat` refuse les groupes déjà coupés.
fn fits(next: Cmd<'_>, rest: &[Cmd<'_>], mut width: isize, must_be_flat: bool) -> bool {
    let mut rest_index = rest.len();
    let mut stack: Vec<(Mode, Item<'_>)> = vec![(next.mode, next.item)];
    while width >= 0 {
        let Some((mode, item)) = stack.pop() else {
            if rest_index == 0 {
                return true;
            }
            rest_index -= 1;
            stack.push((rest[rest_index].mode, rest[rest_index].item));
            continue;
        };
        let doc = match item {
            Item::Concat(parts) | Item::Fill(parts) => {
                for part in parts.iter().rev() {
                    stack.push((mode, Item::Doc(part)));
                }
                continue;
            }
            Item::Doc(doc) => doc,
        };
        match doc {
            Doc::Text(value) => width -= width_of(value) as isize,
            Doc::Concat(parts) | Doc::Fill(parts) => {
                for part in parts.iter().rev() {
                    stack.push((mode, Item::Doc(part)));
                }
            }
            Doc::Indent(contents) => stack.push((mode, Item::Doc(contents))),
            Doc::Group(g) => {
                if must_be_flat && g.should_break {
                    return false;
                }
                let group_mode = if g.should_break { Mode::Break } else { mode };
                let contents = if !g.expanded_states.is_empty() && group_mode == Mode::Break {
                    &g.expanded_states[g.expanded_states.len() - 1]
                } else {
                    &g.contents
                };
                stack.push((group_mode, Item::Doc(contents)));
            }
            Doc::IfBreak(broken, flat) => {
                stack.push((mode, Item::Doc(if mode == Mode::Break { broken } else { flat })));
            }
            Doc::Line(kind) => {
                if mode == Mode::Break || *kind == Line::Hard {
                    return true;
                }
                if *kind == Line::Normal {
                    width -= 1;
                }
            }
            Doc::BreakParent | Doc::Mark(_) => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_break_only_when_needed() {
        let short = group(concat(vec![
            text("["),
            indent(concat(vec![softline(), text("a,"), line(), text("b")])),
            softline(),
            text("]"),
        ]));
        assert_eq!(print(short).text, "[a, b]");
        let long_items: Vec<Doc> = (0..20).map(|i| text(format!("item{i}"))).collect();
        let long = group(concat(vec![
            text("["),
            indent(concat(vec![
                softline(),
                join(&concat(vec![text(","), line()]), long_items),
            ])),
            softline(),
            text("]"),
        ]));
        let printed = print(long).text;
        assert!(printed.starts_with("[\n  item0,\n  item1,"), "{printed}");
        assert!(printed.ends_with("item19\n]"), "{printed}");
    }

    #[test]
    fn hardlines_break_their_groups_and_trim_trailing_spaces() {
        let doc = group(concat(vec![text("a "), hardline(), text("b")]));
        assert_eq!(print(doc).text, "a\nb");
    }

    #[test]
    fn fill_wraps_words() {
        let words: Vec<Doc> =
            "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore"
                .split(' ')
                .map(text)
                .collect();
        let doc = fill(
            words
                .into_iter()
                .enumerate()
                .flat_map(|(i, w)| if i == 0 { vec![w] } else { vec![line(), w] })
                .collect(),
        );
        let printed = print(doc).text;
        assert!(printed.lines().all(|l| l.len() <= PRINT_WIDTH), "{printed}");
        assert_eq!(printed.lines().count(), 2, "{printed}");
    }

    #[test]
    fn marks_record_offsets() {
        let id: NodeId = "n_0000000001".parse().unwrap();
        let printed = print(concat(vec![
            text("<a>"),
            Doc::Mark(Mark::Start(id.clone())),
            text("<b />"),
            Doc::Mark(Mark::End(id.clone())),
        ]));
        assert_eq!(printed.marks, vec![(Mark::Start(id.clone()), 3), (Mark::End(id), 8)]);
    }
}
