use db::WorkspaceDataBase;

use crate::hir_def::{pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope};
use crate::{
    AstId, HasName, HasQualifiers, HirNodeInfo, Qualifier,
    hir_def::{
        expressions::{
            expression::{InitExpr, Integer},
            spec::Spec,
        },
        interned::identifier::Ident,
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct VariableDecl<'db> {
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[tracked]
    pub kind: VariableKind,

    #[tracked]
    pub qualifier: Qualifier,

    #[tracked]
    pub variadic: bool,

    #[tracked]
    pub spec: Spec<'db>,

    #[tracked]
    pub init: Option<InitExpr<'db>>,

    #[tracked]
    pub location: Option<DirectVariable<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for VariableDecl<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for VariableDecl<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }
}

impl<'db> HasQualifiers<'db> for VariableDecl<'db> {
    fn get_qualifiers(&self, db: &'db dyn WorkspaceDataBase) -> Qualifier {
        self.qualifier(db)
    }
}

impl<'db> VariableDecl<'db> {
    pub fn is_input(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Input)
    }

    pub fn is_output(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Output)
    }

    pub fn is_var(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Var)
    }

    pub fn is_in_out(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::InOut)
    }

    pub fn is_external(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::External)
    }

    pub fn is_global(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Global)
    }

    /// Where this declaration's storage lives.
    ///
    /// The one definition of the rule: [`instance_members`] filters on it, and
    /// so does the code generator when it decides whether a resolved name is a
    /// wasm local, a field of the enclosing instance, or a fixed address. They
    /// disagreeing is how a VAR_TEMP once persisted across scans.
    ///
    /// [`instance_members`]: crate::hir_ty::head::inheritance::instance_members
    pub fn storage_class(&self, db: &'db dyn WorkspaceDataBase) -> StorageClass {
        // VAR_EXTERNAL holds no storage of its own; it names a configuration
        // VAR_GLOBAL, which is where the value actually lives.
        if self.is_external(db) || self.is_global(db) {
            return StorageClass::Global;
        }
        // VAR_TEMP is scratch for one call, not instance state, even when the
        // POU that declares it has an instance.
        if self.is_temp(db) {
            return StorageClass::Local;
        }
        match get_scope(db, self.get_scope_id(db)).kind {
            ScopeKind::Pou(Pou::FunctionBlock(_) | Pou::Class(_)) | ScopeKind::Program(_) => {
                StorageClass::InstanceMember
            }
            _ => StorageClass::Local,
        }
    }

    pub fn is_access(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Access)
    }

    pub fn is_temp(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Temp)
    }

    pub fn is_config(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self.kind(db), VariableKind::Config)
    }
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

/// Where a declaration's storage lives — see
/// [`VariableDecl::storage_class`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageClass {
    /// A local of the function being emitted: a FUNCTION's or METHOD's own
    /// variables, and VAR_TEMP anywhere.
    Local,
    /// A field of the enclosing FUNCTION_BLOCK, CLASS or PROGRAM instance.
    InstanceMember,
    /// A configuration VAR_GLOBAL, or the VAR_EXTERNAL naming one.
    Global,
}

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

#[salsa::tracked(debug)]
pub struct DirectVariable<'db> {
    pub adress: Ident,
    pub partly: bool,
    pub offset: Vec<Integer>,
}

impl<'db> DirectVariable<'db> {
    /// The address as written: `%IX0.0`, `%IW4`, `%I*`.
    ///
    /// Neither half names the location on its own — `adress` holds the
    /// prefix and width letters (`IX`) while the numeric parts live in
    /// `offset` — so this is the single renderer for both.
    pub fn to_address(self, db: &'db dyn WorkspaceDataBase) -> String {
        let mut out = String::from("%");
        out.push_str(self.adress(db).text(db));
        if self.partly(db) {
            out.push('*');
            return out;
        }
        for (i, part) in self.offset(db).iter().enumerate() {
            if i > 0 {
                out.push('.');
            }
            out.push_str(part.ident(db).text(db));
        }
        out
    }
    
    pub fn area(self, db: &'db dyn WorkspaceDataBase) -> Option<LocationArea> {
        match self
            .adress(db)
            .text(db)
            .chars()
            .next()?
            .to_ascii_uppercase()
        {
            'I' => Some(LocationArea::Input),
            'Q' => Some(LocationArea::Output),
            'M' => Some(LocationArea::Marker),
            _ => None,
        }
    }
}

/// The three areas a located variable can sit in. Each is one contiguous
/// band in linear memory, so a host copies a whole direction at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LocationArea {
    /// `%I` — written by the host before the scan, never by the program.
    Input,
    /// `%Q` — written by the program, read by the host after the scan.
    Output,
    /// `%M` — the marker area, owned by the program; may be RETAIN.
    Marker,
}

impl LocationArea {
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Input => "%I",
            Self::Output => "%Q",
            Self::Marker => "%M",
        }
    }
}

#[salsa::tracked(debug)]
pub struct LocatedVariable<'db> {
    pub name: Option<Ident>,

    pub located_at: DirectVariable<'db>,

    pub spec: Spec<'db>,

    pub init: Option<InitExpr<'db>>,
}
