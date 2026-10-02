// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A `schema` fence: a tree in box-drawing characters, which GitHub shows as
//! written and the site draws as blocks within blocks, in the front page's
//! style.
//!
//! ```text
//! CONFIGURATION Plant                 the plant
//! ├─ VAR_GLOBAL
//! │  └─ line_speed : UINT := 80
//! └─ RESOURCE Main                    the processor         ◄
//!    └─ TASK …
//! ```
//!
//! - A line's depth is its indent, in steps of three characters.
//! - Two spaces or more part a name from its note.
//! - `◄` at the end of a line puts that part in focus: it is drawn solid,
//!   the blocks holding it as dashed frames, and the rest faded.
//! - A name ending in `…` is a placeholder, drawn dashed.
//! - A leaf that assigns (`:=`, `=>`), or sits in a `VAR_` block or an area
//!   like `%I`, is a line of code, drawn as a chip; any other part is a block.
//! - Blocks whose names start with the same keyword lie side by side, like
//!   two tasks; anything else is stacked.
//! - `──► label` between two parts is an arrow from one to the next. Parts an
//!   arrow joins lie in a row, and in a column on a narrow screen. `▼ label`
//!   is an arrow down: the parts it joins are a column everywhere.
//! - `▒` opens a part of the physical world, drawn hatched: the sensors and
//!   the valves a program only reaches through its addresses.
//! - A name ending in `/` is a folder, drawn as a file browser shows one.
//!   A file name, `axis.st`, is a file: what it holds is drawn as its code.

use crate::highlight::{address_class, escape};

const FOCUS: char = '◄';
const ARROW: char = '►';
const DOWN: char = '▼';
const WORLD: char = '▒';
/// The files a workspace holds as text. A module, `core.wasm`, is not one:
/// it is drawn as a block of what it holds.
const FILE_EXTENSIONS: [&str; 4] = ["st", "toml", "md", "json"];

const FOLDER: &str = "<svg class=\"ico\" viewBox=\"0 0 16 16\" aria-hidden=\"true\"><path d=\"M1.5 3.5h4.5l1.5 1.5h7v8.5h-13z\"/></svg>";
const FILE: &str = "<svg class=\"ico\" viewBox=\"0 0 16 16\" aria-hidden=\"true\"><path d=\"M3.5 1.5h6l3 3v10h-9zM9.5 1.5v3h3\"/></svg>";

struct Node {
    title: String,
    note: String,
    focus: bool,
    children: Vec<Node>,
}

impl Node {
    fn holds_focus(&self) -> bool {
        self.focus || self.children.iter().any(Node::holds_focus)
    }

    fn is_line(&self, parent: Option<&Node>) -> bool {
        self.children.is_empty()
            && (self.title.contains(":=")
                || self.title.contains("=>")
                || parent.is_some_and(|p| {
                    p.title.starts_with("VAR")
                        || p.title.starts_with('%')
                        || p.is_world()
                        || p.children.iter().all(Node::is_plain_name)
                }))
    }

    /// A leaf that is only a name, `Tank$__body__`: no keyword, no file, no
    /// arrow. A list of them reads as a list, not as boxes.
    fn is_plain_name(&self) -> bool {
        self.children.is_empty()
            && self.keyword().is_none()
            && !self.is_arrow()
            && !self.is_file()
            && !self.is_folder()
            && !self.is_world()
            && !self.title.ends_with('…')
    }

    /// `──►` reaches here as `►`: the dashes read as the tree's own.
    fn is_arrow(&self) -> bool {
        self.title.starts_with([ARROW, DOWN])
    }

    fn is_world(&self) -> bool {
        self.title.starts_with(WORLD)
    }

    fn is_folder(&self) -> bool {
        self.title.ends_with('/')
    }

    /// `axis.st`: a name and the extension of a file a workspace holds. A
    /// path to a variable, `Main.L1` or `drive.speed`, looks the same
    /// otherwise.
    fn is_file(&self) -> bool {
        !self.title.contains(' ')
            && self
                .title
                .rsplit_once('.')
                .is_some_and(|(stem, ext)| !stem.is_empty() && FILE_EXTENSIONS.contains(&ext))
    }

    fn keyword(&self) -> Option<&str> {
        self.title.split(' ').next().filter(|w| is_keyword(w))
    }
}

/// Capitals and underscores: `VAR_GLOBAL` is a keyword, `T1` an instance.
/// An area, `%I`, is one too, `%IX0.0` an address.
fn is_keyword(word: &str) -> bool {
    let (shortest, word) = match word.strip_prefix('%') {
        Some(area) => (1, area),
        None => (2, word),
    };
    word.len() >= shortest
        && word.starts_with(|c: char| c.is_ascii_uppercase())
        && word.chars().all(|c| c.is_ascii_uppercase() || c == '_')
}

/// The tree the lines draw. A line indented deeper than one step below the
/// previous is taken as one step: the drawing is never refused, only drawn.
fn parse(src: &str) -> Vec<Node> {
    let mut roots = Vec::new();
    let mut open: Vec<(usize, Node)> = Vec::new();
    fn close(open: &mut Vec<(usize, Node)>, roots: &mut Vec<Node>) {
        let (_, node) = open.pop().expect("an open node");
        match open.last_mut() {
            Some((_, parent)) => parent.children.push(node),
            None => roots.push(node),
        }
    }
    for line in src.lines() {
        let start = line
            .find(|c: char| !matches!(c, '│' | '├' | '└' | '─' | ' '))
            .unwrap_or(line.len());
        let mut rest = line[start..].trim_end();
        if rest.is_empty() {
            continue;
        }
        let focus = rest.ends_with(FOCUS);
        if focus {
            rest = rest.trim_end_matches(FOCUS).trim_end();
        }
        let (title, note) = match rest.find("  ") {
            Some(i) => (&rest[..i], rest[i..].trim()),
            None => (rest, ""),
        };
        let depth = line[..start].chars().count() / 3;
        while open.last().is_some_and(|(d, _)| *d >= depth) {
            close(&mut open, &mut roots);
        }
        let depth = open.last().map_or(0, |(d, _)| depth.min(d + 1));
        open.push((
            depth,
            Node {
                title: title.to_string(),
                note: note.to_string(),
                focus,
                children: Vec::new(),
            },
        ));
    }
    while !open.is_empty() {
        close(&mut open, &mut roots);
    }
    roots
}

/// The figure, on lines with no blank one between them: a blank line would
/// end the HTML block for the Markdown renderer.
pub fn html(src: &str) -> String {
    let roots = parse(src);
    let focused = roots.iter().any(Node::holds_focus);
    let mut out = String::from("<figure class=\"schema\">\n");
    body(&roots, None, focused, false, &mut out);
    out.push_str("</figure>");
    out
}

fn body(nodes: &[Node], parent: Option<&Node>, focused: bool, settled: bool, out: &mut String) {
    let flow = nodes.iter().any(Node::is_arrow);
    let down = nodes.iter().any(|n| n.title.starts_with(DOWN));
    let side_by_side = !flow
        && nodes.len() > 1
        && nodes.iter().all(|n| !n.is_line(parent))
        && nodes[0].keyword().is_some()
        && nodes.iter().all(|n| n.keyword() == nodes[0].keyword());
    out.push_str(if down {
        "<div class=\"sb-body flow down\">\n"
    } else if flow {
        "<div class=\"sb-body flow\">\n"
    } else if side_by_side {
        "<div class=\"sb-body row\">\n"
    } else {
        "<div class=\"sb-body\">\n"
    });
    for node in nodes {
        part(node, parent, focused, settled, out);
    }
    out.push_str("</div>\n");
}

/// `settled` is true inside a part in focus or a faded one: what it holds is
/// drawn plain, since a fade inside a fade would compound.
fn part(node: &Node, parent: Option<&Node>, focused: bool, settled: bool, out: &mut String) {
    if node.is_arrow() {
        let label = node.title.trim_start_matches([ARROW, DOWN]).trim();
        out.push_str(&if label.is_empty() {
            "<div class=\"sb-arrow\"></div>\n".to_string()
        } else {
            format!(
                "<div class=\"sb-arrow\"><span>{}</span></div>\n",
                escape(label)
            )
        });
        return;
    }
    let note = if node.note.is_empty() {
        String::new()
    } else {
        format!("<span class=\"sb-note\">{}</span>", escape(&node.note))
    };
    if node.is_folder() {
        out.push_str(&format!(
            "<div class=\"sfolder\"><div class=\"sfolder-name\">{FOLDER}<span>{}</span>{note}</div>\n",
            escape(&node.title)
        ));
        if !node.children.is_empty() {
            body(&node.children, Some(node), focused, settled, out);
        }
        out.push_str("</div>\n");
        return;
    }
    if node.is_file() {
        out.push_str(&format!(
            "<div class=\"sfile\"><div class=\"sfile-tab\">{FILE}<span>{}</span>{note}</div>\n<div class=\"sfile-code\">\n",
            escape(&node.title)
        ));
        code_lines(&node.children, 0, out);
        out.push_str("</div>\n</div>\n");
        return;
    }
    let mut classes = Vec::new();
    if node.focus {
        classes.push("on");
    } else if focused && !settled {
        classes.push(if node.holds_focus() { "ctx" } else { "off" });
    }
    let settles = settled || classes.first().is_some_and(|c| *c != "ctx");
    if node.title.ends_with('…') {
        classes.push("more");
    }
    if node.is_line(parent) {
        classes.insert(0, "sl");
        out.push_str(&format!(
            "<div class=\"{}\"><code class=\"chip\">{}</code>{note}</div>\n",
            classes.join(" "),
            code_html(&node.title)
        ));
        return;
    }
    classes.insert(0, "sb");
    if node.children.is_empty() {
        classes.insert(1, "leaf");
    }
    if node.is_world() {
        classes.insert(1, "world");
    }
    let mut class = classes.join(" ");
    if let Some(kind) = node.keyword().and_then(kind_class) {
        class.push(' ');
        class.push_str(&kind);
    }
    let title = title_html(node.title.trim_start_matches(WORLD).trim_start());
    out.push_str(&format!(
        "<div class=\"{class}\"><div class=\"sb-head\"><span class=\"sb-title\">{title}</span>{note}</div>\n"
    ));
    if !node.children.is_empty() {
        body(&node.children, Some(node), focused, settles, out);
    }
    out.push_str("</div>\n");
}

/// The class naming what a block or a heading is by its keyword, `k-task`
/// for `TASK`: the stylesheet gives each part of a configuration its color.
/// An area, `%I`, takes the color of its addresses.
pub fn kind_class(keyword: &str) -> Option<String> {
    if let Some(area) = keyword.strip_prefix('%') {
        let kind = match area {
            "I" => "k-input",
            "Q" => "k-output",
            "M" => "k-marker",
            _ => return None,
        };
        return Some(kind.to_string());
    }
    is_keyword(keyword).then(|| format!("k-{}", keyword.to_lowercase().replace('_', "-")))
}

/// A name, its keywords small and bold as the front page sets them, an
/// area (`%I`) or an address in its area's color.
fn title_html(title: &str) -> String {
    title
        .split(' ')
        .map(|w| match (is_keyword(w), address_class(w)) {
            (true, Some(area)) => format!("<span class=\"kw {area}\">{w}</span>"),
            (true, None) => format!("<span class=\"kw\">{w}</span>"),
            (false, Some(area)) => format!("<span class=\"{area}\">{}</span>", escape(w)),
            (false, None) => escape(w),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A line of code, its addresses in their area's color.
fn code_html(text: &str) -> String {
    text.split(' ')
        .map(|w| match address_class(w) {
            Some(area) => format!("<span class=\"{area}\">{}</span>", escape(w)),
            None => escape(w),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a file holds, as the code it reads as: indented, not boxed, a note
/// as a comment.
fn code_lines(nodes: &[Node], depth: usize, out: &mut String) {
    for node in nodes {
        let note = if node.note.is_empty() {
            String::new()
        } else {
            format!(" <span class=\"sb-note\">// {}</span>", escape(&node.note))
        };
        out.push_str(&format!(
            "<div class=\"sfile-line\" style=\"padding-left: {}ch\">{}{note}</div>\n",
            depth * 4,
            title_html(&node.title)
        ));
        code_lines(&node.children, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLANT: &str = "\
CONFIGURATION Plant                 the plant
├─ VAR_GLOBAL
│  └─ line_speed : UINT := 80
└─ RESOURCE Main
   ├─ TASK Fast                     every 10 ms        ◄
   │  └─ PROGRAM …
   └─ TASK Slow                     every 100 ms
";

    #[test]
    fn the_indent_is_the_nesting_and_two_spaces_the_note() {
        let roots = parse(PLANT);
        assert_eq!(roots.len(), 1);
        let plant = &roots[0];
        assert_eq!(
            (plant.title.as_str(), plant.note.as_str()),
            ("CONFIGURATION Plant", "the plant")
        );
        let titles: Vec<_> = plant.children.iter().map(|n| n.title.as_str()).collect();
        assert_eq!(titles, ["VAR_GLOBAL", "RESOURCE Main"]);
        let fast = &plant.children[1].children[0];
        assert_eq!((fast.note.as_str(), fast.focus), ("every 10 ms", true));
        assert_eq!(fast.children[0].title, "PROGRAM …");
    }

    #[test]
    fn focus_frames_what_holds_it_and_fades_the_rest() {
        let out = html(PLANT);
        assert!(out.contains("<div class=\"sb ctx k-configuration\"><div class=\"sb-head\"><span class=\"sb-title\"><span class=\"kw\">CONFIGURATION</span> Plant</span><span class=\"sb-note\">the plant</span>"), "{out}");
        assert!(out.contains("<div class=\"sb on k-task\">"), "{out}");
        assert!(
            out.contains("<div class=\"sb leaf more k-program\">"),
            "inside the focus: {out}"
        );
        assert!(
            out.contains("<div class=\"sb leaf off k-task\">"),
            "TASK Slow: {out}"
        );
        // VAR_GLOBAL fades, and its line with it rather than once more.
        assert!(
            out.contains("<div class=\"sb off k-var-global\"><div class=\"sb-head\"><span class=\"sb-title\"><span class=\"kw\">VAR_GLOBAL</span></span></div>\n<div class=\"sb-body\">\n<div class=\"sl\"><code class=\"chip\">line_speed : UINT := 80</code>"),
            "{out}"
        );
        assert!(
            !out.contains("\n\n"),
            "a blank line ends the HTML block: {out}"
        );
    }

    #[test]
    fn blocks_of_one_keyword_lie_side_by_side() {
        let out = html(PLANT);
        let resource = out.split_once("RESOURCE").unwrap().1;
        assert!(
            resource.contains("<div class=\"sb-body row\">"),
            "the tasks: {out}"
        );
        let (plant, _) = out.split_once("VAR_GLOBAL").unwrap();
        assert!(
            !plant.contains("row"),
            "VAR_GLOBAL and RESOURCE stack: {out}"
        );
    }

    #[test]
    fn only_capitals_are_keywords() {
        let out = html("PROGRAM NON_RETAIN T1 : Tank\n");
        assert!(
            out.contains(
                "<span class=\"kw\">PROGRAM</span> <span class=\"kw\">NON_RETAIN</span> T1 : Tank"
            ),
            "{out}"
        );
    }

    #[test]
    fn an_arrow_joins_the_parts_around_it_in_a_flow() {
        let out = html(
            "%I inputs\n└─ level AT %IW0 : INT\n──► read\nPROGRAM Mixer\n└─ %M markers\n──►\n%Q outputs\n",
        );
        assert!(
            out.starts_with("<figure class=\"schema\">\n<div class=\"sb-body flow\">\n"),
            "{out}"
        );
        assert!(
            out.contains("<span class=\"kw hl-address-input\">%I</span> inputs"),
            "{out}"
        );
        assert!(
            out.contains("<div class=\"sl\"><code class=\"chip\">level AT <span class=\"hl-address-input\">%IW0</span> : INT</code>"),
            "a line of %I: {out}"
        );
        assert!(
            out.contains(
                "<div class=\"sb-arrow\"><span>read</span></div>\n<div class=\"sb k-program\">"
            ),
            "{out}"
        );
        assert!(
            out.contains("<div class=\"sb-arrow\"></div>\n<div class=\"sb leaf k-output\">"),
            "{out}"
        );
        assert!(out.contains("<div class=\"sb leaf k-marker\"><div class=\"sb-head\"><span class=\"sb-title\"><span class=\"kw hl-address-marker\">%M</span> markers"), "a block of its own: {out}");
    }

    #[test]
    fn a_folder_holds_files_and_a_file_holds_its_code() {
        let out = html(
            "src/                  any way you like\n└─ axis.st\n   └─ NAMESPACE Motion\n      └─ FUNCTION_BLOCK Axis    merged\n",
        );
        assert!(
            out.contains("<div class=\"sfolder\"><div class=\"sfolder-name\">"),
            "{out}"
        );
        assert!(
            out.contains("<span>src/</span><span class=\"sb-note\">any way you like</span>"),
            "{out}"
        );
        assert!(
            out.contains("<div class=\"sfile\"><div class=\"sfile-tab\">"),
            "{out}"
        );
        assert!(out.contains("<span>axis.st</span>"), "{out}");
        assert!(out.contains("<div class=\"sfile-line\" style=\"padding-left: 0ch\"><span class=\"kw\">NAMESPACE</span> Motion</div>"), "{out}");
        assert!(out.contains("style=\"padding-left: 4ch\"><span class=\"kw\">FUNCTION_BLOCK</span> Axis <span class=\"sb-note\">// merged</span>"), "{out}");
        assert!(
            !html("%IX0.0\n").contains("sfile") && !html("Main.L1\n").contains("sfile"),
            "not files"
        );
    }

    #[test]
    fn the_world_is_hatched_and_a_down_arrow_stacks() {
        let out = html("▒ the physical world    sensors\n▼ wired to\n%I inputs\n");
        assert!(out.contains("<div class=\"sb-body flow down\">"), "{out}");
        assert!(out.contains("<div class=\"sb world leaf\"><div class=\"sb-head\"><span class=\"sb-title\">the physical world</span>"), "{out}");
        assert!(
            out.contains("<div class=\"sb-arrow\"><span>wired to</span></div>"),
            "{out}"
        );
    }

    #[test]
    fn a_list_of_plain_names_is_drawn_as_chips() {
        let out = html("EXPORT\n├─ __init                every starting value\n└─ Scale\n");
        assert!(out.contains("<div class=\"sl\"><code class=\"chip\">__init</code><span class=\"sb-note\">every starting value</span></div>"), "{out}");
        let mixed = html("MEMORY\n├─ builtins\n└─ %I\n");
        assert!(
            mixed.contains(
                "<div class=\"sb leaf\"><div class=\"sb-head\"><span class=\"sb-title\">builtins"
            ),
            "beside a keyword, a block: {mixed}"
        );
    }

    #[test]
    fn no_focus_draws_everything_plain() {
        let out = html("RESOURCE Main\n├─ TASK Fast\n└─ TASK Slow\n");
        assert!(
            !out.contains("ctx") && !out.contains("off") && !out.contains(" on"),
            "{out}"
        );
    }
}
