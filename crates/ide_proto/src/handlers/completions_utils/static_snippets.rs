use auto_lsp::lsp_types::{self, CompletionItem};

#[inline]
pub fn extends() -> CompletionItem {
    CompletionItem {
        label: "EXTENDS".into(),
        kind: Some(lsp_types::CompletionItemKind::CLASS),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("EXTENDS ${1:ext}".into()),
        ..Default::default()
    }
}

#[inline]
pub fn implements() -> CompletionItem {
    CompletionItem {
        label: "IMPLEMENTS".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("IMPLEMENTS ${1:impl}".into()),
        ..Default::default()
    }
}

#[inline]
pub fn namespace() -> CompletionItem {
    CompletionItem {
        label: "NAMESPACE".into(),
        kind: Some(lsp_types::CompletionItemKind::MODULE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("NAMESPACE ${1:ns} \n\nEND_NAMESPACE".into()),
        ..Default::default()
    }
}

#[inline]
pub fn using() -> CompletionItem {
    CompletionItem {
        label: "USING".into(),
        kind: Some(lsp_types::CompletionItemKind::FOLDER),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("USING ${1:ns}".into()),
        ..Default::default()
    }
}

#[inline]
pub fn function() -> CompletionItem {
    CompletionItem {
        label: "FUNCTION".into(),
        kind: Some(lsp_types::CompletionItemKind::FUNCTION),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("FUNCTION ${1:fn} \n\nEND_FUNCTION".into()),
        ..Default::default()
    }
}

#[inline]
pub fn function_block() -> CompletionItem {
    CompletionItem {
        label: "FUNCTION_BLOCK".into(),
        kind: Some(lsp_types::CompletionItemKind::FUNCTION),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("FUNCTION_BLOCK ${1:fb} \n\nEND_FUNCTION_BLOCK".into()),
        ..Default::default()
    }
}

#[inline]
pub fn type_() -> CompletionItem {
    CompletionItem {
        label: "TYPE".into(),
        kind: Some(lsp_types::CompletionItemKind::TYPE_PARAMETER),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("TYPE \n\nEND_TYPE".into()),
        ..Default::default()
    }
}

#[inline]
pub fn class() -> CompletionItem {
    CompletionItem {
        label: "CLASS".into(),
        kind: Some(lsp_types::CompletionItemKind::CLASS),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("CLASS ${1:cls} \n\nEND_CLASS".into()),
        ..Default::default()
    }
}

#[inline]
pub fn program() -> CompletionItem {
    CompletionItem {
        label: "PROGRAM".into(),
        kind: Some(lsp_types::CompletionItemKind::FUNCTION),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("PROGRAM ${1:prog} \n\nEND_PROGRAM".into()),
        ..Default::default()
    }
}

#[inline]
pub fn interface() -> CompletionItem {
    CompletionItem {
        label: "INTERFACE".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("INTERFACE ${1:iface} \n\nEND_INTERFACE".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var_input() -> CompletionItem {
    CompletionItem {
        label: "VAR_INPUT".into(),
        kind: Some(lsp_types::CompletionItemKind::VARIABLE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR_INPUT \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var_output() -> CompletionItem {
    CompletionItem {
        label: "VAR_OUTPUT".into(),
        kind: Some(lsp_types::CompletionItemKind::VARIABLE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR_OUTPUT \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var_in_out() -> CompletionItem {
    CompletionItem {
        label: "VAR_IN_OUT".into(),
        kind: Some(lsp_types::CompletionItemKind::VARIABLE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR_IN_OUT \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var_temp() -> CompletionItem {
    CompletionItem {
        label: "VAR_TEMP".into(),
        kind: Some(lsp_types::CompletionItemKind::VARIABLE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR_TEMP \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var() -> CompletionItem {
    CompletionItem {
        label: "VAR".into(),
        kind: Some(lsp_types::CompletionItemKind::VARIABLE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn method() -> CompletionItem {
    CompletionItem {
        label: "METHOD".into(),
        kind: Some(lsp_types::CompletionItemKind::METHOD),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("METHOD ${1:meth} \n\nEND_METHOD".into()),
        ..Default::default()
    }
}

#[inline]
pub fn configuration() -> CompletionItem {
    CompletionItem {
        label: "CONFIGURATION".into(),
        kind: Some(lsp_types::CompletionItemKind::MODULE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("CONFIGURATION ${1:config} \n\nEND_CONFIGURATION".into()),
        ..Default::default()
    }
}

#[inline]
pub fn resource() -> CompletionItem {
    CompletionItem {
        label: "RESOURCE".into(),
        kind: Some(lsp_types::CompletionItemKind::MODULE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("RESOURCE ${1:res} ON ${2:resource_type} \n\nEND_RESOURCE".into()),
        ..Default::default()
    }
}

#[inline]
pub fn task_config() -> CompletionItem {
    CompletionItem {
        label: "TASK".into(),
        kind: Some(lsp_types::CompletionItemKind::EVENT),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("TASK ${1:task} (PRIORITY := ${2:0})".into()),
        ..Default::default()
    }
}

#[inline]
pub fn task_single() -> CompletionItem {
    CompletionItem {
        label: "SINGLE".into(),
        kind: Some(lsp_types::CompletionItemKind::PROPERTY),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("SINGLE := ${1:source}".into()),
        ..Default::default()
    }
}

#[inline]
pub fn task_interval() -> CompletionItem {
    CompletionItem {
        label: "INTERVAL".into(),
        kind: Some(lsp_types::CompletionItemKind::PROPERTY),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("INTERVAL := ${1:T#20ms}".into()),
        ..Default::default()
    }
}

#[inline]
pub fn task_priority() -> CompletionItem {
    CompletionItem {
        label: "PRIORITY".into(),
        kind: Some(lsp_types::CompletionItemKind::PROPERTY),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("PRIORITY := ${1:0}".into()),
        ..Default::default()
    }
}

#[inline]
pub fn prog_config() -> CompletionItem {
    CompletionItem {
        label: "PROGRAM (config)".into(),
        kind: Some(lsp_types::CompletionItemKind::MODULE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("PROGRAM ${1:inst} : ${2:prog_type}".into()),
        detail: Some("PROGRAM instance configuration".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var_global() -> CompletionItem {
    CompletionItem {
        label: "VAR_GLOBAL".into(),
        kind: Some(lsp_types::CompletionItemKind::VARIABLE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR_GLOBAL \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn var_access() -> CompletionItem {
    CompletionItem {
        label: "VAR_ACCESS".into(),
        kind: Some(lsp_types::CompletionItemKind::VARIABLE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("VAR_ACCESS \n\nEND_VAR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn struct_() -> CompletionItem {
    CompletionItem {
        label: "STRUCT".into(),
        kind: Some(lsp_types::CompletionItemKind::STRUCT),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("STRUCT\n\t${1:field}: ${2:INT};\nEND_STRUCT".into()),
        ..Default::default()
    }
}

#[inline]
pub fn array() -> CompletionItem {
    CompletionItem {
        label: "ARRAY".into(),
        kind: Some(lsp_types::CompletionItemKind::TYPE_PARAMETER),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("ARRAY[${1:0}..${2:10}] OF ${3:INT}".into()),
        ..Default::default()
    }
}

/// What a VAR section takes: the elementary types and the compound forms,
/// plus `AT` in the sections that map a variable to an address.
pub fn var_section_items(takes_a_location: bool) -> Vec<CompletionItem> {
    let mut items = elem_type_names();
    items.push(struct_());
    items.push(array());
    if takes_a_location {
        items.push(at());
    }
    items
}

/// `AT %IX0.0` — the address a located variable is mapped to. The prefix
/// says the band (input, output, memory) and the letter the width.
#[inline]
pub fn at() -> CompletionItem {
    CompletionItem {
        label: "AT".into(),
        kind: Some(lsp_types::CompletionItemKind::KEYWORD),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("AT %${1|I,Q,M|}${2|X,B,W,D,L|}${3:0}".into()),
        ..Default::default()
    }
}

/// The access specifier a FUNCTION or a METHOD takes between its keyword and
/// its name. A half-typed one parses as the name, so these are offered for as
/// long as no specifier is written yet.
pub fn visibility_names() -> Vec<CompletionItem> {
    ["PUBLIC", "PROTECTED", "PRIVATE", "INTERNAL"]
        .into_iter()
        .map(keyword)
        .collect()
}

#[inline]
pub fn internal() -> CompletionItem {
    keyword("INTERNAL")
}

#[inline]
fn keyword(label: &str) -> CompletionItem {
    CompletionItem {
        label: label.into(),
        kind: Some(lsp_types::CompletionItemKind::KEYWORD),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some(label.into()),
        ..Default::default()
    }
}

#[inline]
pub fn all_stmts() -> Vec<CompletionItem> {
    vec![if_(), for_(), while_(), repeat()]
}

#[inline]
pub fn if_() -> CompletionItem {
    CompletionItem {
        label: "IF".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("IF ${1:cond} THEN \n\nEND_IF".into()),
        ..Default::default()
    }
}

#[inline]
pub fn for_() -> CompletionItem {
    CompletionItem {
        label: "FOR".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("FOR ${1:i} := ${2:value} TO ${3:end} DO \n\nEND_FOR".into()),
        ..Default::default()
    }
}

#[inline]
pub fn while_() -> CompletionItem {
    CompletionItem {
        label: "WHILE".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("WHILE ${1:i} >= ${2:value} DO \n\nEND_WHILE".into()),
        ..Default::default()
    }
}

#[inline]
pub fn repeat() -> CompletionItem {
    CompletionItem {
        label: "REPEAT".into(),
        kind: Some(lsp_types::CompletionItemKind::INTERFACE),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        insert_text: Some("REPEAT \n\n\tUNTIL ${1:i} >= ${2:value} \n\nEND_REPEAT".into()),
        ..Default::default()
    }
}

#[inline]
pub fn elem_type_names() -> Vec<CompletionItem> {
    vec![
        bool(),
        sint(),
        int(),
        dint(),
        lint(),
        usint(),
        uint(),
        udint(),
        ulint(),
        byte(),
        word(),
        dword(),
        lword(),
        date(),
        ldate(),
        dt(),
        ldt(),
        tod(),
        ltod(),
        time(),
        ltime(),
        string(),
        char(),
    ]
}

#[inline]
pub fn elem_type_names_init() -> Vec<CompletionItem> {
    vec![
        sint_init(),
        int_init(),
        dint_init(),
        lint_init(),
        usint_init(),
        uint_init(),
        udint_init(),
        ulint_init(),
        byte_init(),
        word_init(),
        dword_init(),
        lword_init(),
        date_init(),
        ldate_init(),
        dt_init(),
        ldt_init(),
        tod_init(),
        ltod_init(),
        time_init(),
        ltime_init(),
        string_init(),
        char_init(),
    ]
}

use camelpaste::paste;

macro_rules! gen_elem_data_types_snippets {
    ($($type_name: ident => $label: ident = $dv: expr),*) => {
        $(
            #[inline]
            pub fn $type_name() -> CompletionItem {
                CompletionItem {
                    label: stringify!($label).into(),
                    kind: Some(lsp_types::CompletionItemKind::TYPE_PARAMETER),
                    insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                    insert_text: Some(stringify!($label).into()),
                    ..Default::default()
                }
            }

        paste! {
            #[inline]
            pub fn [<$type_name _init>]() -> CompletionItem {
                CompletionItem {
                    label: stringify!($label).into(),
                    kind: Some(lsp_types::CompletionItemKind::VALUE),
                    insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
                    insert_text: Some(format!("{}#{}", stringify!($label), $dv).into()),
                    ..Default::default()
                }
            }}
        )*
    };
}

gen_elem_data_types_snippets! {
    sint => SINT = "0",
    int => INT  = "0",
    dint => DINT  = "0",
    lint => LINT  = "0",
    usint => USINT  = "0",
    uint => UINT  = "0",
    udint => UDINT  = "0",
    ulint => ULINT  = "0",
    byte => BYTE  = "0",
    word => WORD  = "0",
    dword => DWORD  = "0",
    lword => LWORD  = "0",
    date => DATE  = "2024-01-01",
    ldate => LDATE  = "2024-01-01",
    dt => DATE_AND_TIME  = "2024-01-01-00:00:00",
    ldt => LDATE_AND_TIME = "2024-01-01-00:00:00",
    tod => TIME_OF_DAY  = "00:00:00",
    ltod => LTIME_OF_DAY = "00:00:00",
    time => TIME = "0s",
    ltime => LTIME  = "0s",
    bool => BOOL = "FALSE",
    string => STRING = "''",
    char => CHAR = "''"
}
