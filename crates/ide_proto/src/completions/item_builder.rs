use std::fmt::Display;

use auto_lsp::{
    core::span::Span,
    default::db::BaseDatabase,
    lsp_types::{
        self, CompletionItem, CompletionItemKind, CompletionItemLabelDetails, InsertTextFormat, InsertTextMode, Range, TextEdit
    },
};
use hir::{
    HasName, HirNodeInfo, hir_def::{
        expressions::spec::SpecKind,
        interned::namespace::NamespacePath,
        pous::{
            pou::{Pou},
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    }, hir_ty::{name_res::resolve_namespace_access, ty::Type}
};

/// A builder for creating completion items with various options.
#[derive(Default)]
pub struct CompletionBuilder {
    // Whether to include the signature in the completion item.
    // signature should only be used inside statements
    signature: bool,
    // Whether to include an import statement for the completion item.
    // this range indicates where to insert the USING statement 
    import: Option<Range>,
}

impl<'db> CompletionBuilder {
    pub fn with_signature(mut self) -> Self {
        self.signature = true;
        self
    }

    pub fn with_import(mut self, db: &'db dyn BaseDatabase, scope: ScopeId<'db>) -> Self {
        self.import = Some(find_using_range(db, scope));
        self
    }

    pub fn build_variable(
        &self,
        db: &'db dyn BaseDatabase,
        variable: &VariableDecl<'db>,
    ) -> CompletionItem {
        let variable_name = variable.name(db).text(db);
        let insert_text = if self.signature {
            match variable.spec(db).kind(db) {
                SpecKind::Target(t) => resolve_namespace_access(db, &t.path)
                    .filter(|pou| pou.get_scope_id(db).can_have_local_variables(db))
                    .map(|pou| signature(db, &variable_name, pou.get_scope_id(db)))
                    .unwrap_or_else(|| variable_name.to_string()),
                _ => variable_name.to_string(),
            }
        } else {
            variable_name.to_string()
        };

        CompletionItem {
            label: variable_name.to_string(),
            detail: Some(Type::new_spec(db, variable.spec(db)).type_name(db)),
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
        db: &'db dyn BaseDatabase,
        pou: &Pou<'db>,
        namespace: Option<&NamespacePath>,
    ) -> CompletionItem {
        let mut additional_edit = None;
        if let Some(range) = self.import
            && let Some(ns) = namespace
        {
            let namespace_str = ns.to_string(db);
            additional_edit = Some(TextEdit {
                range,
                new_text: format!("\tUSING {namespace_str};\n"),
            });
        }

        let name = pou.get_name_ident(db).text(db).to_string();
        let (detail, kind) = match pou {
            Pou::FunctionBlock(_) => ("(FUNCTION BLOCK)", CompletionItemKind::CLASS),
            Pou::Class(_) => ("(CLASS)", CompletionItemKind::CLASS),
            Pou::Function(_) => ("(FUNCTION)", CompletionItemKind::FUNCTION),
            Pou::DataType(_) => ("(TYPE)", CompletionItemKind::TYPE_PARAMETER),
            Pou::Interface(_) => ("(INTERFACE)", CompletionItemKind::INTERFACE),
        };

        CompletionItem {
            label: name.clone(),
            detail: Some(detail.into()),
            label_details: namespace.map(|ns| CompletionItemLabelDetails {
                detail: Some(format!("(USING {})", ns.to_string(db))),
                description: None,
            }),
            kind: Some(kind),
            insert_text: if self.signature {
                Some(signature(db, &name, pou.get_scope_id(db)))
            } else {
                None
            },
            insert_text_mode: match self.signature {
                true => Some(InsertTextMode::ADJUST_INDENTATION),
                false => None,
            },
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            additional_text_edits: additional_edit.map(|edit| vec![edit]),
            ..Default::default()
        }
    }
}

/// Finds the range where new 'USING' statements should be inserted for the given scope.
pub fn find_using_range<'db>(db: &'db dyn BaseDatabase, node: ScopeId<'db>) -> Range {
    let scope = get_scope(db, node);
    match scope.usings.last() {
        // Some USING directives exist; insert after the last one.
        Some(u) => {
            go_to_next_line(u.get_span(db))
        }
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
            ScopeKind::Namespace(ns) => {
                go_to_next_line(ns.name_span(db))
            }
            ScopeKind::Pou(p) => {
                go_to_next_line(p.get_name_span(db))
            }
            ScopeKind::MethodDecl(m) => {
                // methods can't have USING directives, so we go to the parent scope
                let parent_scope = get_scope(db, m.scope_id(db))
                    .parent
                    .expect("All methods should have a parent scope");
                find_using_range(db, parent_scope)
            }
        },
    }
}

enum IndentMode {
    FollowCurrentLine,
    Indent,
}

// todo: set indentation
#[inline]
fn go_to_next_line(span: Span) -> lsp_types::Range {
    let mut adjusted_span = span.lsp();
    // add one line to both start and end to move to the next line
    adjusted_span.start.line += 1;
    adjusted_span.end.line = adjusted_span.start.line;
    adjusted_span
}

/// Generate the signature snippet for a scope with input/output variables
///
/// THis will return none if the scope has variables
pub fn signature<'db>(
    db: &'db dyn BaseDatabase,
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

    format!("{}({sep}{}{sep});", name, params.join(join_sep))
}
