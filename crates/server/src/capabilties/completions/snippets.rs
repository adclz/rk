use auto_lsp::lsp_types::{self, CompletionItem};

#[inline]
pub fn namespace() -> CompletionItem {
    CompletionItem {
        label: "NAMESPACE".into(),
        kind: Some(lsp_types::CompletionItemKind::MODULE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("NAMESPACE ${1:ns} END_NAMESPACE".into()),
        ..Default::default()
    }
}

#[inline]
pub fn using() -> CompletionItem {
    CompletionItem {
        label: "USING".into(),
        kind: Some(lsp_types::CompletionItemKind::FOLDER),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("USING ${1:ns};".into()),
        ..Default::default()
    }
}

#[inline]
pub fn function() -> CompletionItem {
    CompletionItem {
        label: "FUNCTION".into(),
        kind: Some(lsp_types::CompletionItemKind::FUNCTION),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("FUNCTION ${1:fn} : ${2:BOOL} \n\nEND_FUNCTION".into()),
        ..Default::default()
    }
}

#[inline]
pub fn function_block() -> CompletionItem {
    CompletionItem {
        label: "FUNCTION_BLOCK".into(),
        kind: Some(lsp_types::CompletionItemKind::FUNCTION),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("FUNCTION_BLOCK ${1:fn}  \n\nEND_FUNCTION_BLOCK".into()),
        ..Default::default()
    }
}

#[inline]
pub fn type_() -> CompletionItem {
    CompletionItem {
        label: "TYPE".into(),
        kind: Some(lsp_types::CompletionItemKind::TYPE_PARAMETER),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("TYPE ${1:type} := ${2:BOOL} \n\nEND_TYPE".into()),
        ..Default::default()
    }
}

#[inline]
pub fn class() -> CompletionItem {
    CompletionItem {
        label: "CLASS".into(),
        kind: Some(lsp_types::CompletionItemKind::CLASS),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("CLASS ${1:class} USING ${2:ns} \n\nEND_CLASS".into()),
        ..Default::default()
    }
}

#[inline]
pub fn interface() -> CompletionItem {
    CompletionItem {
        label: "INTERFACE".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("INTERFACE ${1:interface} \n\nEND_INTERFACE".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var_input() -> CompletionItem {
    CompletionItem {
        label: "VAR_INPUT".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR_INPUT \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var_output() -> CompletionItem {
    CompletionItem {
        label: "VAR_OUTPUT".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR_OUTPUT \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var_temp() -> CompletionItem {
    CompletionItem {
        label: "VAR_TEMP".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR_TEMP \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var() -> CompletionItem {
    CompletionItem {
        label: "VAR".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn test() -> CompletionItem {
    CompletionItem {
        label: "TEST".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("INTERFACE ${1:interface} \n;\nEND_INTERFACE".into()),
        ..Default::default()
    }
}