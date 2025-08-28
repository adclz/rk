use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{MarkupContent, MarkupKind},
};

use crate::{
    def::{
        expressions::{expression::InitExpr, spec::Spec},
        interned::identifier::Ident,
        scope::FileScopeId,
        semantic_index::SemanticIndex,
    },
    to_proto::{AstId, SymbolInfo, ToProto},
};

#[salsa::tracked(debug)]
pub struct VariableDecl<'db> {
    #[returns(ref)]
    pub name: Ident,

    #[tracked]
    pub kind: VariableKind,

    #[tracked]
    #[returns(ref)]
    pub spec: Spec<'db>,

    #[tracked]
    #[returns(as_ref)]
    pub init: Option<InitExpr<'db>>,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId,
}

// VAR Internal to entity (function, function block, etc.)
// VAR_INPUT Externally supplied, not modifiable within entity
// VAR_OUTPUT Supplied by entity to external entities
// VAR_IN_OUT Supplied by external entities, can be modified within entity and supplied to external entity
// VAR_EXTERNAL Supplied by configuration via VAR_GLOBAL
// VAR_GLOBAL Global variable declaration
// VAR_ACCESS Access path declaration
// VAR_TEMP Temporary storage for variables in function blocks, methods and programs
// VAR_CONFIG Instance-specific initialization and location assignment.

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VariableKind {
    Var,
    Input,
    Output,
    InOut,
    External,
    Global,
    Access,
    Temp,
    Config,
}

impl<'db> ToProto<'db> for VariableDecl<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        self.scope_id(db)
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::VARIABLE)
                .name(self.name(db).text(db).to_string())
                .range(self.get_span(db).clone())
                .name_range(self.get_name_span(db)?.clone())
                .spec(*self.spec(db))
                .maybe_init(self.init(db).cloned())
                .build(),
        )
    }

    fn hover(
        &'db self,
        db: &'db dyn BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        Some(auto_lsp::lsp_types::Hover {
            contents: auto_lsp::lsp_types::HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("Variable {}", self.name(db).text(db)).to_string(),
            }),
            range: self.get_name_span(db).map(|s| s.lsp()),
        })
    }
}
