#![allow(unused)]
use std::fmt::{Display, format};

use auto_lsp::{
    core::span::Span,
    lsp_types::{
        self, CompletionItem, CompletionItemKind, CompletionItemLabelDetails, InsertTextFormat,
        InsertTextMode, Range, TextEdit,
    },
};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::spec::{Enum, SpecKind},
        interned::namespace::NamespacePath,
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        signature::{infer_signature, inheritance::MethodRef},
        ty::Type,
    },
    query_string::{query::Query, scope::ScopeSearchCtx},
};

use crate::handlers::completions_utils::QueryMode;

/// A builder for creating completion items with various options.
#[derive(Default)]
pub struct CompletionBuilder {
    // Whether to include the signature in the completion item.
    // signatures should only be used inside statements
    mode: QueryMode,
    // Whether to include an import statement for the completion item.
    // this range indicates where to insert the USING statement
    import: Option<(Range, String)>, // Range + indentation
}

impl<'db> CompletionBuilder {
    pub fn with_import(mut self, db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> Self {
        self.import = Some(find_using_range(db, scope));
        self
    }

    pub fn with_mode(mut self, mode: QueryMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn build_variable(
        &self,
        db: &'db dyn WorkspaceDataBase,
        variable: &VariableDecl<'db>,
    ) -> CompletionItem {
        let infer = infer_signature(db, variable.get_scope_id(db));
        let variable_name = variable.name(db).text(db);
        let typ = infer
            .type_of_specs
            .get(&variable.spec(db))
            .copied()
            .unwrap_or_default();

        let insert_text = match self.mode {
            QueryMode::Head => variable_name.to_string(),
            QueryMode::Body => match typ {
                Type::Function(_) | Type::FunctionBlock(_) => {
                    build_call_signature(db, &variable_name, variable.get_scope_id(db))
                }
                _ => variable_name.to_string(),
            },
        };

        CompletionItem {
            label: variable_name.to_string(),
            detail: Some(typ.type_name(db)),
            kind: Some(CompletionItemKind::VARIABLE),
            label_details: Some(CompletionItemLabelDetails {
                detail: Some(
                    match variable.kind(db) {
                        VariableKind::Input => "(INPUT)",
                        VariableKind::Output => "(OUTPUT)",
                        VariableKind::InOut => "(IN_OUT)",
                        VariableKind::Var => "(VAR)",
                        VariableKind::External => "(EXTERNAL)",
                        VariableKind::Global => "(GLOBAL)",
                        VariableKind::Access => "(ACCESS)",
                        VariableKind::Config => "(CONFIG)",
                        VariableKind::Temp => "(TEMP)",
                    }
                    .into(),
                ),
                description: None,
            }),
            insert_text: Some(insert_text),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        }
    }

    pub fn build_pou(
        &self,
        db: &'db dyn WorkspaceDataBase,
        pou: &Pou<'db>,
        namespace: Option<&NamespacePath>,
        items: &mut Vec<CompletionItem>,
    ) {
        // Check if this is an enum type and expand variants instead
        if let Pou::DataType(data_type) = pou
            && let SpecKind::Enum(enm) = data_type.spec(db).kind(db)
        {
            let name = pou.get_name_ident(db).text(db).to_string();
            let additional_edit = self.build_import_edit(db, namespace);
            self.expand_enum_variants(db, &name, *enm, additional_edit, items);
            return;
        }

        // Regular POu handling
        let additional_edit = self.build_import_edit(db, namespace);

        let name = pou.get_name_ident(db).text(db).to_string();
        let (detail, kind) = match pou {
            Pou::FunctionBlock(_) => ("(FUNCTION BLOCK)", CompletionItemKind::CLASS),
            Pou::Class(_) => ("(CLASS)", CompletionItemKind::CLASS),
            Pou::Function(_) => ("(FUNCTION)", CompletionItemKind::FUNCTION),
            Pou::DataType(_) => ("(TYPE)", CompletionItemKind::TYPE_PARAMETER),
            Pou::Interface(_) => ("(INTERFACE)", CompletionItemKind::INTERFACE),
        };

        items.push(CompletionItem {
            label: name.clone(),
            detail: Some(detail.into()),
            label_details: namespace.map(|ns| CompletionItemLabelDetails {
                detail: Some(format!("(USING {})", ns.to_string(db))),
                description: None,
            }),
            kind: Some(kind),
            insert_text: match self.mode {
                QueryMode::Head => None,
                QueryMode::Body => Some(build_call_signature(db, &name, pou.get_scope_id(db))),
            },
            insert_text_mode: match self.mode {
                QueryMode::Head => None,
                QueryMode::Body => Some(InsertTextMode::ADJUST_INDENTATION),
            },
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            additional_text_edits: additional_edit.map(|edit| vec![edit]),
            ..Default::default()
        });
    }

    fn expand_enum_variants(
        &self,
        db: &'db dyn WorkspaceDataBase,
        name: &impl Display,
        enm: Enum<'db>,
        additional_edit: Option<TextEdit>,
        items: &mut Vec<CompletionItem>,
    ) {
        enm.enum_variants(db).iter().for_each(|(_, var)| {
            items.push(CompletionItem {
                label: format!("{}#{}", name, var.name.text(db)),
                detail: Some("(ENUM VARIANT)".into()),
                kind: Some(CompletionItemKind::ENUM_MEMBER),
                additional_text_edits: additional_edit.as_ref().map(|edit| vec![edit.clone()]),
                ..Default::default()
            })
        });
    }

    pub fn build_method(
        &self,
        db: &'db dyn WorkspaceDataBase,
        method: &MethodRef<'db>,
    ) -> CompletionItem {
        let name = method.get_name_ident(db).text(db).to_string();

        CompletionItem {
            label: name.clone(),
            detail: Some("(METHOD)".into()),
            kind: Some(CompletionItemKind::METHOD),
            insert_text: match self.mode {
                QueryMode::Head => None,
                QueryMode::Body => Some(build_call_signature(db, &name, method.get_scope_id(db))),
            },
            insert_text_mode: match self.mode {
                QueryMode::Head => None,
                QueryMode::Body => Some(InsertTextMode::ADJUST_INDENTATION),
            },
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        }
    }

    /// Creates an import TextEdit if both import configuration and namespace are present.
    fn build_import_edit(
        &self,
        db: &'db dyn WorkspaceDataBase,
        namespace: Option<&NamespacePath>,
    ) -> Option<TextEdit> {
        if let Some((range, indent)) = &self.import
            && let Some(ns) = namespace
        {
            let namespace_str = ns.to_string(db);
            Some(TextEdit {
                range: *range,
                new_text: format!("{}USING {namespace_str};\n", indent),
            })
        } else {
            None
        }
    }
}

/// Finds the range where new 'USING' statements should be inserted for the given scope.
/// Returns the range and the appropriate indentation level.
pub fn find_using_range<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: ScopeId<'db>,
) -> (Range, String) {
    let scope = get_scope(db, node);
    let indent = match scope.kind {
        ScopeKind::Global => String::new(),
        _ => "\t".to_string(),
    };

    let range = match scope.usings.last() {
        // Some USING directives exist; insert after the last one.
        Some(u) => go_to_next_line(u.get_span(db)),
        None => match scope.kind {
            // In case of Global scope and no USING directives, insert at the start of the file.
            ScopeKind::Global => lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
            },
            // Same with namespaces and POUs: insert after their name declaration line.
            ScopeKind::Namespace(ns) => go_to_next_line(ns.name_span(db)),
            // We want to avoid inserting USING inside POUs because it could break variable declarations.
            ScopeKind::MethodDecl(_) | ScopeKind::Pou(_) => {
                let parent_scope = scope
                    .parent
                    .expect("All methods should have a parent scope");
                return find_using_range(db, parent_scope);
            }
            _ => unreachable!(),
        },
    };

    (range, indent)
}

// todo: set indentation
#[inline]
fn go_to_next_line(span: Span) -> lsp_types::Range {
    let lsp_span = span.lsp();
    // Move to the beginning of the next line to avoid intersection with existing content
    lsp_types::Range {
        start: lsp_types::Position {
            line: lsp_span.end.line + 1,
            character: 0,
        },
        end: lsp_types::Position {
            line: lsp_span.end.line + 1,
            character: 0,
        },
    }
}

/// Generate the signature snippet for a scope with input/output variables
///
/// THis will return none if the scope has variables
pub fn build_call_signature<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: &impl Display,
    scope: ScopeId<'db>,
) -> String {
    let variables = &scope.def_map(db).local_variables;
    let is_multiline = variables.len() >= 5;
    let (sep, tab, join_sep) = if is_multiline {
        ("\n", "\t", ",\n")
    } else {
        ("", "", ", ")
    };

    let params: Vec<String> = variables
        .iter()
        .enumerate()
        .filter_map(|(i, (_, v))| {
            let placeholder_num = i + 1;
            let var_name = v.name(db).text(db);

            match v.kind(db) {
                VariableKind::Input | VariableKind::InOut => Some(format!(
                    "{tab}{var_name} := ${{{placeholder_num}:{var_name}}}"
                )),
                VariableKind::Output => Some(format!(
                    "{tab}{var_name} => ${{{placeholder_num}:{var_name}}}"
                )),
                _ => None,
            }
        })
        .collect();

    format!("{}({sep}{}{sep})", name, params.join(join_sep))
}
