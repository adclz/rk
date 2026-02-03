use std::fmt::Display;

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
        interned::namespace::NamespacePath,
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{signature::infer_signature, ty::Type},
    query_string::scope::query_scope_items,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryMode {
    Signature,
    Body,
}

pub struct ScopeCompletionCtx<'db> {
    pub scope: ScopeId<'db>,
    pub mode: QueryMode,
    pub offset: usize,
    pub query: &'db str,
    pub items: Vec<CompletionItem>,
}

impl<'db> ScopeCompletionCtx<'db> {
    pub fn new(mode: QueryMode, scope: ScopeId<'db>, offset: usize, query: &'db str) -> Self {
        Self {
            mode,
            scope,
            offset,
            query,
            items: vec![],
        }
    }

    pub fn take_items(&mut self) -> Vec<CompletionItem> {
        std::mem::take(&mut self.items)
    }

    pub fn query_scope_items(&mut self, db: &'db dyn WorkspaceDataBase) {
        let builder = CompletionBuilder::default().with_import(db, self.scope);

        let builder = match self.mode {
            QueryMode::Body => builder.with_signature(),
            QueryMode::Signature => builder,
        };

        // Filter pous based on the query mode
        // If Signature, we assume it's a datatype or variable declaration
        // everything but functions should be suggested

        // If Body, Functions are allowed but not other POUs
        // that's because they have to be declared in var sections
        let filter = match self.mode {
            QueryMode::Signature => |pou: &Pou<'db>| {
                !matches!(pou, Pou::Function(_))
            },
            QueryMode::Body => |pou: &Pou<'db>| {
                matches!(pou, Pou::Function(_))
            },

        };

        let pous = query_scope_items(db, self.query, self.scope, filter);

        for pou in pous.local_pous {
            self.items.push(builder.build_pou(db, &pou, None));
        }

        for (ns, pou) in pous.need_imports {
            self.items.push(builder.build_pou(db, &pou, Some(&ns)));
        }

        for var in pous.local_variables {
            self.items.push(builder.build_variable(db, &var));
        }
    }
}

/// A builder for creating completion items with various options.
#[derive(Default)]
pub struct CompletionBuilder {
    // Whether to include the signature in the completion item.
    // signature should only be used inside statements
    signature: bool,
    // Whether to include an import statement for the completion item.
    // this range indicates where to insert the USING statement
    import: Option<(Range, String)>, // Range + indentation
}

impl<'db> CompletionBuilder {
    pub fn with_signature(mut self) -> Self {
        self.signature = true;
        self
    }

    pub fn with_import(mut self, db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> Self {
        self.import = Some(find_using_range(db, scope));
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

        let insert_text = if self.signature {
            match typ {
                Type::Function(_) | Type::FunctionBlock(_) => {
                    signature(db, &variable_name, variable.get_scope_id(db))
                }
                _ => variable_name.to_string(),
            }
        } else {
            variable_name.to_string()
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
    ) -> CompletionItem {
        let mut additional_edit = None;
        if let Some((range, indent)) = &self.import
            && let Some(ns) = namespace
        {
            let namespace_str = ns.to_string(db);
            additional_edit = Some(TextEdit {
                range: *range,
                new_text: format!("{}USING {namespace_str};\n", indent),
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
pub fn signature<'db>(
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

    format!("{}({sep}{}{sep});", name, params.join(join_sep))
}
