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
        // VAR_GLOBAL, which is where the value actually lives. A PROGRAM's
        // located VAR is its channel's cell, like a located VAR_GLOBAL.
        if self.is_external(db) || self.is_global(db) || self.is_program_located(db) {
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

    /// A PROGRAM's `VAR` located at a complete address (`x AT %IX0.0`). It is
    /// the channel, not a field of the instance: every instance of the
    /// program reads and writes the one cell.
    pub fn is_program_located(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_var(db)
            && self
                .location(db)
                .is_some_and(|dv| crate::hir_ty::infer::normalize::names_a_band(db, dv))
            && matches!(
                get_scope(db, self.get_scope_id(db)).kind,
                ScopeKind::Program(_)
            )
    }

    /// Declared `AT %I*`, `%Q*` or `%M*`: its address is left to the
    /// configuration, which gives each instance its own in VAR_CONFIG.
    pub fn is_partly_located(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.location(db).is_some_and(|dv| dv.partly(db))
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

    /// The size character: the letter after the area, or `X` when it is left
    /// out — `%I1` is a bit, Table 16 row 4b. `None` for anything longer
    /// than an area and a size, which names no width.
    pub fn size_letter(self, db: &'db dyn WorkspaceDataBase) -> Option<char> {
        let mut letters = self.adress(db).text(db).chars();
        letters.next()?;
        match (letters.next(), letters.next()) {
            (None, _) => Some('X'),
            (Some(size), None) => Some(size),
            (Some(_), Some(_)) => None,
        }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::Update)]
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

/// One I/O address, reduced to what identifies its channel: the area, the
/// width its size character names, and its levels. Only built for an address
/// that names a band, so everything here has storage.
///
/// The text is upper-cased, which is also how two mentions of one address
/// are recognised as one: lowering names the cell of a bare address the same
/// way.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, salsa::Update)]
pub struct LocatedAddress {
    pub area: LocationArea,
    /// What the size character names, in bits: 1, 8, 16, 32 or 64.
    pub width: u8,
    /// The numeric levels, in order; `u32::MAX` for one too large to fit.
    pub levels: Vec<u32>,
    /// The address as written, upper-cased.
    pub text: compact_str::CompactString,
}

impl LocatedAddress {
    /// The bits of its area the address covers, read as a byte-addressed
    /// image: each size counts in its own units, so `%IWn` is bytes 2n and
    /// 2n+1 and `%IDn` bytes 4n to 4n+3, and `%IXn.b` is bit b of byte n.
    /// That is CODESYS's numbering, where every narrower address lies inside
    /// exactly one wider one and two of the same size never overlap.
    ///
    /// `None` for an address with no such reading — one level for a bit, two
    /// for anything wider, three or more, or a bit past 7 — which is a cell
    /// of its own.
    pub fn image_bits(&self) -> Option<std::ops::Range<u64>> {
        match (self.width, self.levels.as_slice()) {
            (1, &[byte, bit]) if bit < 8 => {
                let at = u64::from(byte) * 8 + u64::from(bit);
                Some(at..at + 1)
            }
            (1, _) => None,
            (width, &[n]) => {
                let at = u64::from(n) * u64::from(width);
                Some(at..at + u64::from(width))
            }
            _ => None,
        }
    }

    /// The address `dv` names, or `None` when it names no band (E1417).
    pub fn of<'db>(db: &'db dyn WorkspaceDataBase, dv: DirectVariable<'db>) -> Option<Self> {
        if !crate::hir_ty::infer::normalize::names_a_band(db, dv) {
            return None;
        }
        let (_, bits) = dv
            .size_letter(db)
            .and_then(crate::hir_ty::infer::normalize::access_size)?;
        Some(Self {
            area: dv.area(db)?,
            width: bits as u8,
            levels: dv
                .offset(db)
                .iter()
                .map(|part| {
                    part.ident(db)
                        .text(db)
                        .replace('_', "")
                        .parse::<u32>()
                        .unwrap_or(u32::MAX)
                })
                .collect(),
            text: compact_str::CompactString::from(dv.to_address(db).to_ascii_uppercase()),
        })
    }
}

#[salsa::tracked(debug)]
pub struct LocatedVariable<'db> {
    pub name: Option<Ident>,

    pub located_at: DirectVariable<'db>,

    pub spec: Spec<'db>,

    pub init: Option<InitExpr<'db>>,
}
