use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, CompletionItemKind, InsertTextFormat, ParameterInformation, ParameterLabel,
        SignatureHelp, SignatureInformation,
    },
};
use rustc_hash::FxHashMap;

use crate::{
    hir_def::interned::identifier::Ident,
    hir_ty::ty::{Ty, TyKind},
};

impl<'db> Ty<'db> {
    pub fn to_signature(&self, db: &'db dyn BaseDatabase) -> Option<CallableSignature<'db>> {
        match self.kind(db) {
            TyKind::Function {
                input,
                output,
                in_out,
                return_type,
            } => Some(CallableSignature {
                origin: *self,
                inputs: input.clone(),
                in_outs: in_out.clone(),
                outputs: output.clone(),
                return_type,
            }),
            TyKind::FunctionBlock {
                extends,
                inputs,
                outputs,
                in_outs,
            } => Some(CallableSignature {
                origin: *self,
                inputs: inputs.clone(),
                in_outs: in_outs.clone(),
                outputs: outputs.clone(),
                return_type: None,
            }),
            TyKind::Method {
                is_prototype,
                input,
                output,
                in_out,
            } => Some(CallableSignature {
                origin: *self,
                inputs: input.clone(),
                in_outs: in_out.clone(),
                outputs: output.clone(),
                return_type: None,
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct CallableSignature<'db> {
    pub origin: Ty<'db>,
    pub inputs: FxHashMap<Ident, Ty<'db>>,
    pub in_outs: FxHashMap<Ident, Ty<'db>>,
    pub outputs: FxHashMap<Ident, Ty<'db>>,
    pub return_type: Option<Ty<'db>>,
}

impl<'db> CallableSignature<'db> {
    pub fn get_param(&self, ident: &Ident) -> Option<&Ty<'db>> {
        self.inputs
            .get(ident)
            .or_else(|| self.outputs.get(ident))
            .or_else(|| self.in_outs.get(ident))
    }

    pub fn to_completion_item(&self, db: &'db dyn BaseDatabase) -> CompletionItem {
        let mut params = vec![];
        for (name, ty) in &self.inputs {
            params.push(format!("{} := $", name.text(db)));
        }
        for (name, ty) in &self.in_outs {
            params.push(format!("{} := $", name.text(db)));
        }
        for (name, ty) in &self.outputs {
            params.push(format!("{} => $", name.text(db)));
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

    pub fn to_signature(&self, db: &'db dyn BaseDatabase) -> SignatureHelp {
        let sig = SignatureInformation {
            label: self.origin.decl(db).name(db).text(db).to_string(),
            parameters: Some(
                self.inputs
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
