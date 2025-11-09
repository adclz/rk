use auto_lsp::{
    core::semantic_tokens_builder::SemanticTokensBuilder,
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, GotoDefinitionResponse, Hover, SemanticTokenType,
        request::GotoDeclarationResponse,
    },
};
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::spec::{Spec, SpecKind},
        pous::pou::{Pou, PouDecl},
    },
    hir_ty::{
        inheritance_solver::inherited_methods,
        name_res::resolve_namespace_access,
        walk::{ResolvedPath, ResolvedPathKind},
    },
};

use crate::{
    CLASS, FUNCTION, INTERFACE, SUPPORTED_TYPES, ToProtocol, VARIABLE,
};

impl<'db> ToProtocol<'db> for ResolvedPath<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        match &self.kind {
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::This(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p) => p.hover(db, p.name_span(db).start_byte),
            ResolvedPathKind::Spec(t) => t.hover(db, offset),
            ResolvedPathKind::StructElement(st) => st.hover(db, offset),
            ResolvedPathKind::Variable(v) => v.hover(db, offset),
            ResolvedPathKind::Method(m) => m.hover(db, offset),
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match &self.kind {
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::This(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p) => p.declaration(db),
            ResolvedPathKind::Spec(t) => t.declaration(db),
            ResolvedPathKind::StructElement(st) => st.declaration(db),
            ResolvedPathKind::Variable(v) => v.declaration(db),
            ResolvedPathKind::Method(m) => m.declaration(db),
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match &self.kind {
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::This(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p) => p.definition(db),
            ResolvedPathKind::Spec(t) => t.definition(db),
            ResolvedPathKind::StructElement(st) => st.definition(db),
            ResolvedPathKind::Variable(v) => v.definition(db),
            ResolvedPathKind::Method(m) => m.definition(db),
        }
    }

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        Some(match self.kind {
            ResolvedPathKind::Variable(v) => v.completion(db, offset)?,
            ResolvedPathKind::Pou(pou) => pou.completion(db, offset)?,
            ResolvedPathKind::Spec(spec) => spec.completion(db, offset)?,
            ResolvedPathKind::This(pou) => pou
                .scope_id(db)
                .def_map(db)
                .declared_methods
                .iter()
                .filter_map(|(_, m)| m.completion(db, offset))
                .flatten()
                .collect::<Vec<_>>(),
            ResolvedPathKind::Super(pou) => inherited_methods(db, pou)
                .methods
                .iter()
                .filter_map(|(_, m)| m.method.completion(db, offset))
                .flatten()
                .collect::<Vec<_>>(),
            _ => None?,
        })
    }

    fn semantic_tokens(&'db self, db: &'db dyn BaseDatabase, builder: &mut SemanticTokensBuilder) {
        match self.kind {
            ResolvedPathKind::Pou(p) => {
                if let Some((typ, modi)) = p.tokens(db) {
                    builder.push(
                        self.get_span(db).lsp(),
                        SUPPORTED_TYPES.iter().position(|x| *x == typ).unwrap() as u32,
                        modi,
                    );
                }
            }
            ResolvedPathKind::Variable(v) => {
                match v.spec(db).tokens(db) {
                    Some((typ, modi)) => {
                        builder.push(
                            self.get_span(db).lsp(),
                            SUPPORTED_TYPES.iter().position(|x| *x == typ).unwrap() as u32,
                            modi,
                        );
                    }
                    None => {
                        builder.push(
                            self.get_span(db).lsp(),
                            SUPPORTED_TYPES.iter().position(|x| *x == VARIABLE).unwrap() as u32,
                            0,
                        );
                    }
                };
            }
            _ => {}
        }
    }
}

pub trait HasTokens<'db> {
    fn tokens(&'db self, db: &'db dyn BaseDatabase) -> Option<(SemanticTokenType, u32)>;
}

impl<'db> HasTokens<'db> for PouDecl<'db> {
    fn tokens(&'db self, db: &'db dyn BaseDatabase) -> Option<(SemanticTokenType, u32)> {
        let typ = match self.pou(db) {
            Pou::FunctionBlock(p) => FUNCTION,
            Pou::Function(p) => FUNCTION,
            Pou::Interface(p) => INTERFACE,
            Pou::Class(p) => CLASS,
            Pou::DataType(p) => return p.spec(db).tokens(db),
        };

        return Some((typ, 0));
    }
}

impl<'db> HasTokens<'db> for Spec<'db> {
    fn tokens(&'db self, db: &'db dyn BaseDatabase) -> Option<(SemanticTokenType, u32)> {
        let typ = match self.kind(db) {
            SpecKind::Target(t) => match resolve_namespace_access(db, &t.path) {
                Some(pou) => return pou.tokens(db),
                None => None?,
            },
            _ => None?,
        };

        return Some((typ, 0));
    }
}
