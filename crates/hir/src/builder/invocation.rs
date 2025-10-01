use auto_lsp::anyhow;

use crate::{
    builder::{
        expression::{ParseExpr, ParseExpression, ParseVariableAccess},
        semantic_index::SemanticIndexBuilder,
    },
    check::errors::analysis_error::AnalysisError,
    hir_def::{
        expressions::{
            expression::{ParamAssign, ParamAssignKind},
            invocation::{Invocation, InvocationKind},
        },
        interned::identifier::SpanIdent,
    },
};

pub trait ParseInvocation<'db> {
    fn to_invocation(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Invocation<'db>, AnalysisError<'db>>;
}

impl<'db> ParseInvocation<'db> for ast::generated::SuperBodyInvocation {
    fn to_invocation(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Invocation<'db>, AnalysisError<'db>> {
        Ok(Invocation::new(
            sema.db,
            self.into(),
            self.SUPER.cast(&sema.ast).into(),
            sema.current_scope,
            InvocationKind::SuperBody,
            vec![], 
        ))
    }
}

impl<'db> ParseInvocation<'db> for ast::generated::Invocation {
    fn to_invocation(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Invocation<'db>, AnalysisError<'db>> {
        let keyword_id;
        let kind = match &self.invocation.cast(&sema.ast) {
            ast::generated::SuperInvocation_ThisInvocation::ThisInvocation(func_call) => {
                keyword_id = func_call.THIS.cast(&sema.ast).into();
                InvocationKind::This {
                    path: func_call.children.cast(&sema.ast).parse(sema)?,
                }
            }
            ast::generated::SuperInvocation_ThisInvocation::SuperInvocation(func_call) => {
                keyword_id = func_call.SUPER.cast(&sema.ast).into();
                InvocationKind::Super {
                    path: func_call.children.cast(sema.ast).parse(sema)?,
                }
            } 
        };

        let mut parameters = vec![];
        for params in self.params.iter() {
            match params.cast(sema.ast) {
                ast::generated::Comma_ParamAssign::Token_Comma(_) => {}
                ast::generated::Comma_ParamAssign::ParamAssign(p) => {
                    match p.children.cast(sema.ast) {
                        ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignInput(p) => {
                            parameters.push(ParamAssign::new(
                                sema.db,
                                p.into(),
                                sema.current_scope,
                                match p.param.as_ref() {
                                    Some(param) => ParamAssignKind::FormalInput {
                                        param: SpanIdent::from_node(
                                            sema.db,
                                            sema,
                                            param.cast(sema.ast),
                                        )?,
                                        value: p.value.cast(sema.ast).to_expr(sema)?,
                                    },
                                    None => ParamAssignKind::NonFormal {
                                        value: p.value.cast(sema.ast).to_expr(sema)?,
                                    },
                                },
                            ))
                        }
                        ast::generated::ParamAssignInput_ParamAssignOutput::ParamAssignOutput(
                            p,
                        ) => {
                            let variable = p.variable.cast(sema.ast).to_access(sema)?;

                            parameters.push(ParamAssign::new(
                                sema.db,
                                p.into(),
                                sema.current_scope,
                                ParamAssignKind::FormalOutput {
                                    not: p.not.is_some(),
                                    param: SpanIdent::from_node(
                                        sema.db,
                                        sema,
                                        p.param.cast(sema.ast),
                                    )?,
                                    variable,
                                },
                            ))
                        }
                    }
                }
            }
        }

        Ok(Invocation::new(
            sema.db,
            self.into(),
            keyword_id,
            sema.current_scope,
            kind,
            parameters,
        ))
    }
}
