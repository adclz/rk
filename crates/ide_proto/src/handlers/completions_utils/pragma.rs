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

/// The pragmas that annotate a declaration, offered where the declarations
/// themselves are.
pub fn pou_pragmas(braces: Braces) -> Vec<CompletionItem> {
    vec![
        item("test", "", braces),
        item("extern", " '${1:module}' '${2:name}'", braces),
        item("once", "", braces),
        item("warn", " = '${1:message}'", braces),
        item("info", " = '${1:message}'", braces),
        item("allow", " '${1:rule}'", braces),
    ]
}

/// The pragmas that stand where a statement stands.
pub fn stmt_pragmas(braces: Braces) -> Vec<CompletionItem> {
    vec![
        item("wasm", " '${1:i32.add}'", braces),
        item("allow", " '${1:rule}'", braces),
    ]
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
