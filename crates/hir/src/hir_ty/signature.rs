use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, CompletionItemKind, InsertTextFormat, ParameterInformation, ParameterLabel,
        SignatureHelp, SignatureInformation,
    },
};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    builder::variables, hir_def::interned::identifier::Ident, hir_ty::ty::{Ty, TyKind}
};

impl<'db> Ty<'db> {
    pub fn to_signature(&self, db: &'db dyn BaseDatabase) -> Option<CallableSignature<'db>> {
        match self.kind(db) {
            TyKind::Function {
                variables,
                return_type,
            } => {
                Some(CallableSignature {
                    origin: *self,
                    variables: variables.clone(),
                    return_type,
                })
            }
            TyKind::FunctionBlock {
                extends,
                variables
            } => {
                Some(CallableSignature {
                    origin: *self,
                    variables: variables.clone(),
                    return_type: None,
                })
            }
            TyKind::Method {
                is_prototype,
                variables
            } => {
                Some(CallableSignature {
                    origin: *self,
                    variables: variables.clone(),
                    return_type: None,
                })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableSignature<'db> {
    pub origin: Ty<'db>,
    // We use IndexMap here to preserve the order of parameters.
    // Otherwise it'd be complicated to resolve non-formal parameters.
    pub variables: IndexMap<Ident, Ty<'db>>,
    pub return_type: Option<Ty<'db>>, 
}

impl<'db> CallableSignature<'db> {
    pub fn to_completion_item(&self, db: &'db dyn BaseDatabase) -> CompletionItem {
        let mut params = vec![];
        for (name, ty) in &self.variables {
            if ty.is_variable_input(db) || ty.is_variable_inout(db) {
                params.push(format!("{} := $", name.text(db)));
            } else if ty.is_variable_output(db) {
                params.push(format!("{} => $", name.text(db)));
            }
        }

        let name = self.origin.decl(db).name(db).text(db).to_string();
        let params = params.join(", \n");

        CompletionItem {
            label: name.to_owned(),
            kind: Some(match self.origin.kind(db) {
                TyKind::Function { .. } | TyKind::FunctionBlock { .. } => {
                    CompletionItemKind::FUNCTION
                }
                _ => CompletionItemKind::METHOD,
            }),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            insert_text: Some(format!(r#"{name}(${params});"#)),
            ..Default::default()
        }
    }

    pub fn length(&self) -> usize {
        self.variables.len()
    }

    pub fn to_signature(&self, db: &'db dyn BaseDatabase) -> SignatureHelp {
        let sig = SignatureInformation {
            label: self.origin.decl(db).name(db).text(db).to_string(),
            parameters: Some(
                self.variables
                    .iter()
                    .map(|(name, ty)| ParameterInformation {
                        label: ParameterLabel::Simple(name.text(db).to_string()),
                        documentation: None,
                    })
                    .collect(),
            ),
            documentation: None,
            active_parameter: None,
        };
        SignatureHelp {
            signatures: vec![sig],
            active_signature: None,
            active_parameter: None,
        }
    }
}
