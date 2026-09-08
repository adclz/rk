use auto_lsp::lsp_types::{self, CompletionItem};

/// Where the cursor sits relative to a pragma's braces. The editor
/// auto-closes `{`, so the closing brace is usually already written and
/// completing it again would leave `{test}}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Braces {
    /// `{test|}` — write the name alone.
    Closed,
    /// `{test|` — write the name and close it.
    Open,
}

/// Whether `offset` sits in the name of a pragma being typed: a `{` opens it
/// with nothing but the name in between. A cursor past the name, in the
/// arguments, is not completing the pragma itself.
pub fn braces_at(source: &str, offset: usize) -> Option<Braces> {
    let before = source.get(..offset)?;
    let opened = before
        .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '#')
        .ends_with('{');
    if !opened {
        return None;
    }
    let after = source.get(offset..)?.trim_start_matches([' ', '\t']);
    Some(if after.starts_with('}') {
        Braces::Closed
    } else {
        Braces::Open
    })
}

/// Whether the cursor sits in the quoted argument of an `{allow}`, which
/// names a lint rule. The argument is a plain string, so nothing in the tree
/// tells it apart from any other; what precedes it in the source does.
pub fn allow_rule_at(source: &str, offset: usize) -> bool {
    let Some(before) = source.get(..offset) else {
        return false;
    };
    let Some(open) = before.rfind('{') else {
        return false;
    };
    let inside = &before[open + 1..];
    // A closing brace means that pragma is behind us, not around us.
    if inside.contains('}') {
        return false;
    }
    let named = inside.trim_start();
    let Some(rest) = named.strip_prefix("allow") else {
        return false;
    };
    if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    // Inside the quotes rather than between them: an odd count so far.
    inside.matches('\'').count() % 2 == 1
}

/// Every lint rule an `{allow}` can name. Read from the linter so the list
/// cannot fall behind the rules it offers.
pub fn allow_rules() -> Vec<CompletionItem> {
    linter::rules::ALL_RULE_NAMES
        .iter()
        .map(|name| CompletionItem {
            label: (*name).to_string(),
            kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
            ..Default::default()
        })
        .collect()
}

/// The pragmas that annotate a declaration, offered where the declarations
/// themselves are.
pub fn pou_pragmas(braces: Braces) -> Vec<CompletionItem> {
    vec![
        item("test", "", braces),
        item("extern", " '${1:module}' '${2:name}'", braces),
        item("once", "", braces),
        item("warn", " = '${1:message}'", braces),
        item("info", " = '${1:message}'", braces),
        allow(braces),
    ]
}

/// The pragmas that stand where a statement stands.
pub fn stmt_pragmas(braces: Braces) -> Vec<CompletionItem> {
    vec![item("wasm", " '${1:i32.add}'", braces), allow(braces)]
}

/// `{allow}` writes its argument as a snippet CHOICE over every rule, so the
/// editor offers them the moment the pragma lands, the way `AT` offers its
/// bands. A choice is drawn by the editor from the snippet itself, asking
/// nothing of the server, which is what makes it unconditional.
fn allow(braces: Braces) -> CompletionItem {
    let rules = linter::rules::ALL_RULE_NAMES.join(",");
    item("allow", &format!(" '${{1|{rules}|}}'"), braces)
}

/// The opening brace is what triggered the completion, so it is never
/// written again. Filtering is on the bare word, which is what follows it.
fn item(name: &str, args: &str, braces: Braces) -> CompletionItem {
    let close = match braces {
        Braces::Closed => "",
        Braces::Open => "}",
    };
    CompletionItem {
        label: format!("{{{name}}}"),
        filter_text: Some(name.into()),
        kind: Some(lsp_types::CompletionItemKind::KEYWORD),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some(format!("{name}{args}{close}")),
        ..Default::default()
    }
}
