use auto_lsp::default::db::BaseDatabase;

use crate::hir::{
    expressions::expression::{ParamAssign, PathExpr},
    pous::variable::VariableKind,
    semantic_index::SemanticIndex,
    signature::{CallableSignature, CallableSignatureIter},
};

/// Will be moved to dedicated module later

impl<'db> CallableSignature<'db> {
    pub fn iter(&'db self, db: &'db dyn BaseDatabase) -> CallableSignatureIter<'db> {
        CallableSignatureIter {
            signature: self,
            db,
            input_iter: self.input_section.iter(),
            output_iter: self.output_section.iter(),
            in_out_iter: self.in_out_section.iter(),
            current_section: Some(VariableKind::Input),
        }
    }
}

impl<'db> CallableSignature<'db> {
    pub fn check_signature(
        &self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        path: PathExpr<'db>,
        params: Vec<ParamAssign<'db>>,
    ) {
        let iterable = self.iter(db);
        let mut itera = params.iter();

        for (name, param, section) in iterable {
            match itera.next() {
                Some(ParamAssign::ParamAssignInput { param, value }) => {
                    match section {
                        VariableKind::Input | VariableKind::InOut => {
                            if let Some(p_name) = param {
                                if p_name != name {
                                    // Error: parameter name mismatch
                                }
                            }

                            // TYPE CHECK
                        }
                        _ => {
                            // Error: attempting to assign output parameter in input section
                        }
                    }
                }
                Some(ParamAssign::ParamAssignOutput {
                    not,
                    param,
                    variable,
                }) => {
                    match section {
                        VariableKind::Output => {
                            if param != name {
                                // Error: parameter name mismatch
                            }

                            // TYPE CHECK
                        }
                        _ => {
                            // Error: attempting to assign input parameter in output section
                        }
                    }
                }
                None => {
                    // Error: not enough parameters provided
                }
            }
        }

        if let Some(param) = itera.next() {
            // Error: too many parameters provided
        }
    }
}
