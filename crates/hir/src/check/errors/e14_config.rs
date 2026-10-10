// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::config::ConfigDecl;
use crate::hir_def::expressions::expression::PathExpr;
use crate::hir_def::expressions::spec::Spec;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_ty::ty::Type;
use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::lsp_types::DiagnosticTag;
use auto_lsp::tree_sitter;
use auto_lsp::tree_sitter::Range;
use db::WorkspaceDataBase;
use db::workspace::Workspace;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ConfigError<'db> {
    NoConfigFileFound {
        file: File,
    },
    /// The workspace declares more than one CONFIGURATION. One workspace
    /// builds one PLC, and a POU is a type usable in any of them, so a second
    /// configuration makes "which globals are in scope here" unanswerable.
    MultipleConfigurations {
        config: ConfigDecl<'db>,
        /// Every OTHER configuration, so they can be reached from here.
        others: Vec<ConfigDecl<'db>>,
    },
    /// The configuration declares more than one RESOURCE. A resource is its
    /// own execution unit and a runtime drives one, so a second resource
    /// compiled into the module was refused at DEPLOY, from a compile that
    /// exited 0; refusing here is the same rule, said where it can be fixed.
    /// TEMPORARY: lifts when multi-resource deployment lands (one runtime
    /// instance per RESOURCE).
    MultipleResources {
        config: ConfigDecl<'db>,
        /// Every resource name across the configuration's fragments, sorted.
        names: Vec<compact_str::CompactString>,
        span: tree_sitter::Range,
    },
    /// A TASK or PROGRAM declared directly in a CONFIGURATION.
    TaskOrProgramOutsideResource(Range),
    MissingPriority(Range),
    /// A TASK's PRIORITY is not a number this compiler can represent. Held as
    /// source text until here, so an unusable value would otherwise reach the
    /// scheduler as "no priority" and quietly sort last.
    InvalidPriority {
        task: SpanIdent<'db>,
        value: Ident,
    },
    IntervalAfterPriority(Range),
    SingleAfterInterval(Range),
    SingleAfterPriority(Range),
    /// A TASK the scheduler cannot honour. `reason` says which rule it broke,
    /// so the four causes do not collapse into one message.
    UnschedulableTask {
        task: SpanIdent<'db>,
        reason: UnschedulableReason,
    },
    /// The task name referenced in a `WITH <task>` clause does not exist in the config.
    UnknownTaskRef {
        task: SpanIdent<'db>,
    },
    /// A PROGRAM instance carries no `WITH <task>`, so nothing would ever run it.
    ProgramWithoutTask {
        instance: SpanIdent<'db>,
    },
    /// A VAR_CONFIG path's first segment doesn't match any program instance in the configuration.
    ConfigInstInitUnknownInstance {
        instance_name: SpanIdent<'db>,
    },
    /// A VAR_CONFIG path references a field that doesn't exist on the resolved type.
    ConfigInstInitFieldNotFound {
        field: SpanIdent<'db>,
        parent_type: Type<'db>,
    },
    /// A VAR_ACCESS declaration's type does not match the referenced variable's actual type.
    AccessDeclTypeMismatch {
        var_origin: VariableDecl<'db>,
        spec: Spec<'db>,
        expected: Type<'db>,
        actual: Type<'db>,
    },
    /// An element of a program configuration's list, `PROGRAM P1 WITH T :
    /// F(x1 := src, y1 => snk, fb1 WITH T2)`, that cannot hold.
    ProgElementRefused {
        expr: PathExpr<'db>,
        why: ProgElementRefusal<'db>,
    },
    /// A direct variable used anywhere: read or written in a body, or named
    /// by a declaration's `AT` clause. The address is TYPED — `X/B/W/D/L`
    /// names the width — but nothing maps it to an I/O image
    DirectVariableUnsupported {
        site: CallSite<'db>,
        /// The address AS WRITTEN
        address: compact_str::CompactString,
        why: UnlocatableAddress,
    },
    /// A partial access whose size character names no slice: `w.%Z1`. The
    /// grammar cannot catch it, because `adress_identifier` is shared with
    /// direct variables and so admits any letters. Refusing it here is what
    /// keeps it out of lowering, which has no reading for it and used to fail
    /// with an internal compiler error on a program `check` had accepted.
    UnknownMultibitsAccess {
        expr: PathExpr<'db>,
        /// The size character AS WRITTEN.
        access: compact_str::CompactString,
    },
    /// A partial access past the end of its base: `w.%X16` or `w.%D0` of a
    /// WORD.
    MultibitsOutOfRange {
        expr: PathExpr<'db>,
        /// The declaration to point at, when the base IS one. A slice of a
        /// struct field or an array element has no declaration of its own.
        var: Option<VariableDecl<'db>>,
        offset: usize,
        /// Width in bits of one slice - 1 for `%X`, 8 for `%B`, and so on.
        access_bits: usize,
        /// Largest offset the base type admits, or `None` when the base is too
        /// narrow to hold even one slice (`%D` on a `WORD`) - there is no valid
        /// offset then, so reporting a range would contradict itself.
        max_offset: Option<usize>,
        base_type: Type<'db>,
    },
    /// A partial access of what is not a bit string, an integer or a BOOL:
    /// `r.31` of a REAL, `a.0` of an ARRAY.
    PartialAccessWithoutParts {
        expr: PathExpr<'db>,
        /// The declaration to point at, when the base IS one.
        var: Option<VariableDecl<'db>>,
        base_type: Type<'db>,
    },
    /// A recursive call whose frame is larger than the stack `stack_size`
    /// sets: it could never be pushed, and the program would stop at the
    /// first such call.
    FrameLargerThanStack {
        site: CallSite<'db>,
        /// What pushes it: "each call of 'f'".
        what: String,
        frame: u64,
        stack: u64,
    },
    /// A write to a variable declared `AT` an input address. The host owns
    /// the input band: it copies the process image in before the scan, so a
    /// store the program makes is overwritten before anyone can read it.
    /// Silently accepting it produced a program whose assignments vanished.
    WriteToInputLocation {
        site: CallSite<'db>,
        /// The address AS WRITTEN.
        address: compact_str::CompactString,
        via: InputWriteRoute,
    },
    /// `RETAIN` on a variable located in `%I` or `%Q`. The retain band is
    /// restored at startup; restoring an input image means the first scan
    /// runs on the values of the last power cycle, before the field bus has
    /// refreshed them. `%M` is the area that may legitimately persist.
    RetainOnIoLocation {
        var: VariableDecl<'db>,
        /// The address AS WRITTEN.
        address: compact_str::CompactString,
    },
    /// Two declarations bound to the same address. Each gets a cell of its
    /// own, so the program has two variables where the plant has one channel:
    /// a host binding by address finds the same address twice and has to
    /// write both, and a write through one is invisible through the other.
    DuplicateLocation {
        var: VariableDecl<'db>,
        /// Another declaration at the address: each of them is reported,
        /// since the files they are in have no order.
        other: VariableDecl<'db>,
        /// The address AS WRITTEN, by `var`.
        address: compact_str::CompactString,
    },
    /// A located variable holds one value of the width its size letter
    /// names, so it is declared as an elementary type of that width.
    /// `AT %IX0.0 : INT` names one bit and declares sixteen; `AT %IW0 :
    /// Colour` or `: ARRAY[..] OF ..` declares no single width at all — the
    /// host binds a channel of the width the address says, and the program
    /// would read something else.
    LocationWidthMismatch {
        var: VariableDecl<'db>,
        /// The address AS WRITTEN.
        address: compact_str::CompactString,
        /// What the size letter names, in bits.
        address_bits: usize,
        /// The declared type's width in bits, or `None` when it is not an
        /// elementary type with one: an aggregate, an enum, a subrange, a
        /// STRING.
        declared_bits: Option<usize>,
        declared: Type<'db>,
    },
    /// An address used in a way only storage of its own allows, when it is
    /// part of a wider address the workspace mentions — `%QX0.3` beside a
    /// `%QW0` is that word's bit 3, with no address of its own to hand out.
    PartOfWiderAddress {
        site: CallSite<'db>,
        /// The address, upper-cased.
        address: compact_str::CompactString,
        /// The address it is part of.
        owner: compact_str::CompactString,
        usage: WiderAddressUse,
    },
    /// A VAR_CONFIG entry's `AT`, which cannot locate the variable it names.
    ConfigLocationRefused {
        expr: PathExpr<'db>,
        /// The variable the path names.
        var: Ident,
        /// The address as written in the entry.
        address: compact_str::CompactString,
        why: ConfigLocationRefusal<'db>,
    },
    /// A variable declared `AT %I*`, `%Q*` or `%M*` that VAR_CONFIG does not
    /// locate, or cannot.
    PartlyLocatedUnlocated(PartlyUnlocated<'db>),
    /// A VAR_CONFIG entry that resolves but cannot be taken as written.
    ConfigEntryRefused {
        expr: PathExpr<'db>,
        why: ConfigEntryRefusal<'db>,
    },
    /// `RETAIN` on an instance that holds a variable declared `AT %M*`. The
    /// variable points at its marker and has no storage of its own, so
    /// retaining the instance would silently leave its count behind.
    RetainHoldsPartlyLocated {
        var: VariableDecl<'db>,
        /// The member declared with the partial address: `x`, or `fb.x`.
        member: Vec<Ident>,
    },
    /// A variable declared `AT %I*`, `%Q*` or `%M*` named in an instance's
    /// initializer, `d : Drive := (out := 30)`. The variable points at the
    /// channel VAR_CONFIG gives its instance, and the value would be written
    /// over the pointer.
    PartlyLocatedOverwritten {
        site: CallSite<'db>,
        /// The member declared with the partial address: `x`, or `fb.x`.
        member: Vec<Ident>,
        address: compact_str::CompactString,
    },
    /// An instance copied whole, `b := a` or `fb(o => b)`, with a member
    /// declared `AT %I*`, `%Q*` or `%M*`. The member points at the channel
    /// VAR_CONFIG gives its instance, and the copy would write the other
    /// instance's pointer over it.
    PartlyLocatedCopied {
        /// The target of the copy.
        site: CallSite<'db>,
        /// The member declared with the partial address: `x`, or `fb.x`.
        member: Vec<Ident>,
        address: compact_str::CompactString,
    },
}

/// Why a VAR_CONFIG entry cannot be taken as written.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ConfigEntryRefusal<'db> {
    /// `Res.P1.arr[0]`, `Res.P1.p^.x`: a path names instances and variables.
    PathStep,
    /// The type the entry repeats is not the variable's.
    TypeMismatch {
        var: Ident,
        written: Type<'db>,
        declared: Type<'db>,
    },
    /// Another entry gives the same variable a value too, or the instance
    /// holding it.
    ValueTwice { var: Ident },
    /// A value for a variable at an address a declaration names: the
    /// declaration gives that channel its starting value.
    ChannelDeclared {
        var: Ident,
        address: compact_str::CompactString,
    },
    /// A value for the PROGRAM instance itself rather than a variable of it.
    ProgramValue,
}

/// Why a VAR_CONFIG entry cannot give the variable it names this address.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ConfigLocationRefusal<'db> {
    /// The variable's declaration has an address of its own, or none.
    NotPartlyLocated,
    /// Declared `AT %I*` and given a `%Q` address, for instance.
    AreaMismatch {
        declared: compact_str::CompactString,
    },
    /// `%I*` again, or no area or width letter.
    Unlocatable { incomplete: bool },
    /// The address is not as wide as the variable's type.
    Width {
        address_bits: usize,
        declared_bits: Option<usize>,
        declared: Type<'db>,
    },
    /// A bit inside a wider address the workspace names, which has no address
    /// of its own for the variable to point at.
    BitOfWider { owner: compact_str::CompactString },
    /// Another entry locates the same instance's variable.
    LocatedTwice { other: compact_str::CompactString },
}

impl<'db> ConfigLocationRefusal<'db> {
    fn message(&self, db: &'db dyn WorkspaceDataBase, var: &str, address: &str) -> String {
        match self {
            Self::NotPartlyLocated => format!(
                "'{var}' is not declared AT %I*, %Q* or %M*, so its address is not VAR_CONFIG's to give"
            ),
            Self::AreaMismatch { declared } => {
                format!("'{var}' is declared AT {declared}, while '{address}' is in another area")
            }
            Self::Unlocatable { incomplete: true } => {
                format!("'{address}' is not a complete address")
            }
            Self::Unlocatable { incomplete: false } => {
                format!("'{address}' does not name an area and a width")
            }
            Self::Width {
                address_bits,
                declared_bits,
                declared,
            } => {
                let bits = |n: usize| {
                    if n == 1 {
                        "1 bit".to_string()
                    } else {
                        format!("{n} bits")
                    }
                };
                let declared = declared.type_name(db);
                match declared_bits {
                    Some(n) => format!(
                        "'{var}' is '{declared}' ({}), while '{address}' is {}",
                        bits(*n),
                        bits(*address_bits)
                    ),
                    None => format!(
                        "'{var}' is '{declared}', not an elementary type, while '{address}' is {}",
                        bits(*address_bits)
                    ),
                }
            }
            Self::BitOfWider { owner } => {
                format!("'{address}' is a bit of '{owner}', with no address to locate '{var}' at")
            }
            Self::LocatedTwice { other } => {
                format!("'{var}' is located at '{address}' here and at '{other}' by another entry")
            }
        }
    }

    /// The rule behind the refusal, and what to write instead.
    fn advice(&self) -> (Option<&'static str>, Option<&'static str>) {
        match self {
            Self::NotPartlyLocated => (
                None,
                Some(
                    "declare it AT %I*, %Q* or %M* in its POU to leave its address to the configuration",
                ),
            ),
            Self::AreaMismatch { .. } => (
                Some("the area is the declaration's"),
                Some("give an input an address in %I, an output one in %Q, a marker one in %M"),
            ),
            Self::Unlocatable { .. } => (
                Some("VAR_CONFIG gives the complete address, such as '%IX0.0' or '%QW4'"),
                None,
            ),
            Self::Width { .. } => (
                Some("the variable holds one value as wide as its address"),
                Some("give it an address of its type's width"),
            ),
            Self::BitOfWider { .. } => (
                Some("the variable points at its channel"),
                Some("give it a byte or wider, or a bit that no wider address around it names"),
            ),
            Self::LocatedTwice { .. } => (
                Some("an instance's variable has one address"),
                Some("keep one of the entries"),
            ),
        }
    }
}

/// A variable declared `AT %I*`, `%Q*` or `%M*` that is left without an
/// address.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum PartlyUnlocated<'db> {
    /// An instance whose variable no VAR_CONFIG entry locates.
    Missing {
        /// The PROGRAM instance, where the CONFIGURATION declares it.
        instance: SpanIdent<'db>,
        /// The members from the instance: `fb.x` of `P1.fb.x`.
        path: Vec<Ident>,
        /// `%I*`, `%Q*` or `%M*`.
        address: compact_str::CompactString,
    },
    /// A FUNCTION's or METHOD's result, made for each call.
    Returned {
        ret: crate::hir_def::expressions::spec::Spec<'db>,
        /// `FUNCTION` or `METHOD`.
        callable: &'static str,
        /// The returned type.
        ty: Type<'db>,
        /// The member declared with the partial address: `x`, or `fb.x`.
        member: Vec<Ident>,
        address: compact_str::CompactString,
    },
    /// Instances held where no VAR_CONFIG path reaches them.
    Unreachable {
        var: VariableDecl<'db>,
        /// The member declared with the partial address: `x`, or `fb.x`.
        member: Vec<Ident>,
        address: compact_str::CompactString,
        place: UnreachablePlace,
    },
}

/// Where an instance is held that VAR_CONFIG cannot name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum UnreachablePlace {
    /// An element of an array: a VAR_CONFIG path names instances, not
    /// elements.
    Array,
    /// A VAR_GLOBAL: a VAR_CONFIG path starts at a PROGRAM instance.
    Global,
    /// A FUNCTION's, a METHOD's, or a VAR_TEMP: a new instance per call.
    PerCall,
    /// A field of a STRUCT: a VAR_CONFIG path names instances, not fields.
    Struct,
    /// A VAR_INPUT: each call copies its argument over it, pointers included.
    Input,
}

/// What a part of a wider address was used for that it cannot be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum WiderAddressUse {
    /// Passed to a VAR_IN_OUT, which takes an address.
    InOut,
    /// Given to `REF()`, which takes an address.
    Reference,
    /// Declared RETAIN: persistence belongs to storage, and this has none.
    Retain(OwnerDeclaration),
    /// Given an initial value: `__init` writes storage, and this has none,
    /// so the value was silently dropped.
    Initializer(OwnerDeclaration),
}

/// What names the address a part belongs to, which decides where its
/// RETAIN or initial value can go instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum OwnerDeclaration {
    /// A VAR_GLOBAL or a PROGRAM's VAR is located at it.
    Declared,
    /// Only a VAR_CONFIG entry, which gives it to a variable declared
    /// `AT %I*`, `%Q*` or `%M*`.
    Configured,
    /// Only a body, bare.
    Bare,
}

impl WiderAddressUse {
    fn message(&self, address: &str, owner: &str) -> String {
        let is = format!("'{address}' is part of '{owner}'");
        match self {
            Self::InOut => format!("{is} and has no address of its own to pass to a VAR_IN_OUT"),
            Self::Reference => format!("{is} and has no address of its own to take a reference to"),
            Self::Retain(_) => format!("{is} and cannot be RETAIN on its own"),
            Self::Initializer(_) => format!("{is} and cannot have an initial value of its own"),
        }
    }

    /// The rule behind the refusal, and what to write instead.
    fn advice(&self, owner: &str) -> (Option<String>, Option<String>) {
        let persist =
            format!("to persist this part, declare a VAR_GLOBAL located at '{owner}', RETAIN");
        let initialize = format!(
            "declare a VAR_GLOBAL located at '{owner}' with this part set in its initial value"
        );
        let bare = format!("no variable is located at '{owner}'");
        match self {
            Self::InOut => (
                None,
                Some("copy it into a variable, pass that, and assign it back".to_string()),
            ),
            Self::Reference => (
                None,
                Some(format!("take the reference of '{owner}' as a whole")),
            ),
            Self::Retain(OwnerDeclaration::Declared) => (
                None,
                Some(format!("declare the variable located at '{owner}' RETAIN")),
            ),
            Self::Retain(OwnerDeclaration::Configured) => (
                Some(format!(
                    "VAR_CONFIG gives '{owner}' to a variable declared AT %M*, with no storage to retain"
                )),
                Some(persist),
            ),
            Self::Retain(OwnerDeclaration::Bare) => (Some(bare), Some(persist)),
            Self::Initializer(OwnerDeclaration::Declared) => (
                None,
                Some(format!(
                    "give the variable located at '{owner}' an initial value with this part set in it"
                )),
            ),
            Self::Initializer(OwnerDeclaration::Configured) => (
                Some(format!(
                    "VAR_CONFIG gives '{owner}' to a variable that takes no initial value"
                )),
                Some(initialize),
            ),
            Self::Initializer(OwnerDeclaration::Bare) => (Some(bare), Some(initialize)),
        }
    }
}

/// Why an address has no storage to be given. Each has its own fix, so each
/// says its own: one message covering all three said only that the address
/// was unsupported, which was true of none of them once the bands landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum UnlocatableAddress {
    /// An `AT` clause on a variable of a POU other than a PROGRAM.
    InPou,
    /// `%I*`: the address is deliberately incomplete.
    Incomplete,
    /// No area letter, or no width letter — `%I1` is Table 16 row 4b, where
    /// the size character is omitted and means BOOL.
    Malformed,
    /// `%IW*`, `%Z*`: a partial address names an area, I, Q or M, and
    /// nothing else.
    NotAreaOnly,
    /// An address in a library file. A library is code for any machine, and
    /// its addresses were not the workspace's: a library's `%QD0` got a cell
    /// of its own beside a workspace `%QW0`.
    InLibrary,
    /// A complete address on a STRUCT field, which was accepted and
    /// ignored: a field is part of every variable of its type. rk has no
    /// relative addresses either, which IEC 61131-3 allows there.
    InStruct,
    /// An incomplete address on a STRUCT field (`AT %Q*`), also accepted and
    /// ignored. IEC 61131-3 lets VAR_CONFIG complete it per variable; rk
    /// does that for a FUNCTION_BLOCK's or CLASS's variables only.
    InStructPartly,
}

impl UnlocatableAddress {
    fn message(self, address: &str) -> String {
        match self {
            Self::InPou => format!("'{address}' cannot locate a variable of this POU"),
            Self::Incomplete => format!("'{address}' is not a complete address"),
            Self::Malformed => format!("'{address}' does not name an area and a width"),
            Self::NotAreaOnly => {
                format!("'{address}' is not a partial address: write '%I*', '%Q*' or '%M*'")
            }
            Self::InLibrary => format!("'{address}' cannot be named in a library"),
            Self::InStruct | Self::InStructPartly => {
                format!("'{address}' cannot locate a STRUCT field")
            }
        }
    }

    /// The rule behind the refusal, and what to write instead. Neither for a
    /// library's address: whoever sees it uses the library and cannot rewrite
    /// it.
    fn advice(self) -> (Option<&'static str>, Option<&'static str>) {
        match self {
            Self::InPou => (
                Some(
                    "the variables of a FUNCTION, FUNCTION_BLOCK or CLASS belong to each call or instance",
                ),
                Some(
                    "declare it AT %I*, %Q* or %M* and locate each instance in VAR_CONFIG, or declare it in a PROGRAM or as a VAR_GLOBAL",
                ),
            ),
            Self::Incomplete => (
                Some(
                    "VAR_CONFIG completes a partial address for a variable of a PROGRAM, FUNCTION_BLOCK or CLASS, instance by instance",
                ),
                Some("write the address in full"),
            ),
            Self::Malformed => (
                Some("an address names its area with I, Q or M and its width with X, B, W, D or L"),
                Some("write it as '%IX0.0', or as '%I0.0' for a bit"),
            ),
            Self::NotAreaOnly => (
                Some("the variable's type gives the width, and VAR_CONFIG the rest of the address"),
                None,
            ),
            Self::InLibrary => (None, None),
            Self::InStruct => (
                Some("a field is part of every variable of its type"),
                Some(
                    "declare the address on a variable of a PROGRAM or a VAR_GLOBAL, and copy between it and the field",
                ),
            ),
            Self::InStructPartly => (
                Some("an incomplete address on a STRUCT field is not supported"),
                Some(
                    "declare the channel in a FUNCTION_BLOCK, where VAR_CONFIG locates it per instance",
                ),
            ),
        }
    }
}

/// How a program reached an input it may not write. Assignment covers every
/// route that ends in a store — `:=`, a FOR control variable, an output
/// binding (`o => sensor`), a partial write (`sensor.3 := TRUE`) and a
/// VAR_IN_OUT or VAR_OUTPUT argument. A reference is the one route that does
/// not store yet: it hands out the capability to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum InputWriteRoute {
    Assignment,
    Reference,
    /// An initial value: `__init` writes it, and the host's copy-in before
    /// the first scan overwrites it before anything reads it.
    Initializer,
}

/// Why an element of a program configuration's list cannot hold.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ProgElementRefusal<'db> {
    /// `name := source` on a variable that is not a VAR_INPUT.
    NotAnInput { var: Ident },
    /// `name => sink` on a variable that is not a VAR_OUTPUT.
    NotAnOutput { var: Ident },
    /// A VAR_GLOBAL whose type is not the variable's.
    TypeMismatch {
        var: Ident,
        declared: Type<'db>,
        global: Ident,
        ty: Type<'db>,
    },
    /// An address whose width is not the variable's.
    WidthMismatch {
        var: Ident,
        declared: Type<'db>,
        address: compact_str::CompactString,
        bits: u8,
    },
    /// A source or sink that names no VAR_GLOBAL.
    NoSuchGlobal { name: Ident },
    /// An input connected twice.
    ConnectedTwice { var: Ident },
    /// An element that names more than a variable of the program.
    NotAVariable,
    /// `fb WITH task` on a variable that is not a FUNCTION_BLOCK instance.
    NotAFunctionBlock { var: Ident },
    /// A function block associated with a task twice.
    AssociatedTwice { var: Ident },
    /// `fb WITH task` naming no task of the resource.
    UnknownTask { task: Ident },
    /// `fb WITH task` on an instance with an `ARRAY[*]` VAR_IN_OUT, which
    /// only a call binds.
    UnboundConformand { var: Ident, param: Ident },
    /// A function block a task runs, which the program's body calls too.
    CalledByProgram {
        var: Ident,
        program: Ident,
        call: crate::CallSite<'db>,
    },
}

/// Why a TASK cannot be scheduled. Only cyclic tasks with a literal, non-zero
/// INTERVAL are; each other shape gets its own message rather than a shared
/// "unsupported", because the fix differs in every case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum UnschedulableReason {
    /// `SINGLE := <event>` — event-driven tasks are not implemented.
    EventDriven,
    /// Neither SINGLE nor INTERVAL. Nothing triggers the task, so a program
    /// bound to it never runs.
    NoTrigger,
    /// INTERVAL names something whose value is not fixed at compile time — a
    /// non-CONSTANT global, a directly represented variable, an unknown name.
    /// A CONSTANT global IS accepted; the period just has to be knowable.
    NonLiteralInterval,
    /// INTERVAL is zero, which describes no cadence at all.
    ZeroInterval,
}

impl UnschedulableReason {
    fn message(self) -> &'static str {
        match self {
            Self::EventDriven => "event-driven tasks (SINGLE) are not supported",
            Self::NoTrigger => "a TASK needs an INTERVAL to run its programs",
            Self::NonLiteralInterval => {
                "INTERVAL must be a TIME literal or a CONSTANT global holding one"
            }
            Self::ZeroInterval => "INTERVAL must be greater than zero",
        }
    }

    /// The follow-up a user needs to actually fix it. The message says what is
    /// wrong; this says what to write instead.
    fn help(self) -> Option<&'static str> {
        match self {
            Self::EventDriven => Some("use a cyclic period, e.g. `INTERVAL := T#10ms`"),
            Self::NoTrigger => Some("add `INTERVAL := T#10ms`"),
            // The likeliest cause is a global that is simply not marked
            // CONSTANT — the value looks fixed right there in the source, so
            // without this the rejection reads as arbitrary.
            Self::NonLiteralInterval => Some("declare the global `VAR_GLOBAL CONSTANT`"),
            Self::ZeroInterval => None,
        }
    }
}

impl<'db> ErrorCode for ConfigError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::NoConfigFileFound { .. } => "E1401",
            Self::MultipleConfigurations { .. } => "E1402",
            Self::MultipleResources { .. } => "E1403",
            Self::TaskOrProgramOutsideResource(_) => "E1404",
            Self::MissingPriority(_) => "E1405",
            Self::InvalidPriority { .. } => "E1406",
            Self::IntervalAfterPriority(_) => "E1407",
            Self::SingleAfterInterval(_) => "E1408",
            Self::SingleAfterPriority(_) => "E1409",
            Self::UnschedulableTask { .. } => "E1410",
            Self::UnknownTaskRef { .. } => "E1411",
            Self::ProgramWithoutTask { .. } => "E1412",
            Self::ConfigInstInitUnknownInstance { .. } => "E1413",
            Self::ConfigInstInitFieldNotFound { .. } => "E1414",
            Self::AccessDeclTypeMismatch { .. } => "E1415",
            Self::DirectVariableUnsupported { .. } => "E1417",
            Self::UnknownMultibitsAccess { .. } => "E1418",
            Self::MultibitsOutOfRange { .. } => "E1429",
            Self::FrameLargerThanStack { .. } => "E1430",
            Self::PartialAccessWithoutParts { .. } => "E1431",
            Self::WriteToInputLocation { .. } => "E1419",
            Self::RetainOnIoLocation { .. } => "E1420",
            Self::DuplicateLocation { .. } => "E1421",
            Self::LocationWidthMismatch { .. } => "E1422",
            Self::PartOfWiderAddress { .. } => "E1423",
            Self::ConfigLocationRefused { .. } => "E1424",
            Self::PartlyLocatedUnlocated(_) => "E1425",
            Self::ConfigEntryRefused { .. } => "E1426",
            Self::RetainHoldsPartlyLocated { .. } => "E1420",
            Self::PartlyLocatedOverwritten { .. } => "E1427",
            Self::PartlyLocatedCopied { .. } => "E1427",
            Self::ProgElementRefused { .. } => "E1428",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for ConfigError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::NoConfigFileFound { file } => {
                let mut diag = diag()
                    .message("the workspace has no config.toml".to_string())
                    .severity(DiagnosticSeverity::HINT)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &file.document(db).tree.root_node().range())
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_help(format!(
                    "add a config.toml with a [project] table in '{}'",
                    Workspace::get(db)
                        .workspace_folder(db)
                        .map(|w| w.to_string_lossy())
                        .unwrap_or_default()
                ));

                diag
            }
            Self::MultipleConfigurations { config, others } => {
                let mut diag = diag()
                    .message(format!(
                        "the workspace declares {} CONFIGURATIONs",
                        others.len() + 1
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &config.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();

                for other in others {
                    diag.with_related(Related::new(
                        format!("'{}' is declared here", other.name_with_case(db).text(db)),
                        other.get_scope_id(db).file(db),
                        other.get_name_span(db),
                    ));
                }
                diag.with_note("a workspace has one CONFIGURATION".to_string());
                diag.with_help("describe another PLC in its own workspace".to_string());
                diag
            }
            Self::MultipleResources {
                config,
                names,
                span,
            } => {
                let list = names
                    .iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = config;
                let mut diag = diag()
                    .message(format!(
                        "the configuration declares {} RESOURCEs ({list})",
                        names.len(),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                diag.with_note("a deployment drives one RESOURCE".to_string());
                diag.with_help("deploy one RESOURCE per runtime".to_string());
                diag
            }
            Self::TaskOrProgramOutsideResource(span) => {
                let mut diag = diag()
                    .message("a TASK or a PROGRAM instance is declared inside a RESOURCE".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(
                            db,
                            file,
                            &crate::check::errors::first_word(db, file, span),
                        )
                        .unwrap_or_default(),
                    )
                    .call();

                diag.with_help(
                    "wrap them in a RESOURCE <name> ON <cpu> ... END_RESOURCE block".into(),
                );
                diag
            }
            Self::MissingPriority(span) => diag()
                .message("the TASK has no PRIORITY".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::InvalidPriority { task, value } => {
                let mut diag = diag()
                    .message(format!(
                        "task '{}' has an unusable PRIORITY '{}'",
                        task.with_case.text(db),
                        value.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &task.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note("PRIORITY is a 32-bit unsigned integer, 0 the most urgent".into());
                diag
            }
            Self::IntervalAfterPriority(span) => diag()
                .message("INTERVAL cannot be declared after PRIORITY".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::SingleAfterInterval(span) => diag()
                .message("SINGLE cannot be declared after INTERVAL".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::SingleAfterPriority(span) => diag()
                .message("SINGLE cannot be declared after PRIORITY".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::UnschedulableTask { task, reason } => {
                let mut diag = diag()
                    .message(format!(
                        "task '{}' cannot be scheduled: {}",
                        task.with_case.text(db),
                        reason.message()
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &task.get_span(db)).unwrap_or_default())
                    .call();
                if let Some(help) = reason.help() {
                    diag.with_help(help.to_string());
                }
                diag
            }
            Self::UnknownTaskRef { task } => diag()
                .message(format!(
                    "no TASK is named '{}' in this configuration",
                    task.with_case.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &task.get_span(db)).unwrap_or_default())
                .call(),
            Self::ProgramWithoutTask { instance } => diag()
                .message(format!(
                    "program instance '{}' has no WITH <task>, so it will never run",
                    instance.with_case.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &instance.get_span(db)).unwrap_or_default())
                .call(),
            Self::ConfigInstInitUnknownInstance { instance_name } => diag()
                .message(format!(
                    "no PROGRAM instance is named '{}' in this configuration",
                    instance_name.as_str(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(
                    crate::denormalize(db, file, &instance_name.get_span(db)).unwrap_or_default(),
                )
                .call(),
            Self::ConfigInstInitFieldNotFound { field, parent_type } => diag()
                .message(format!(
                    "'{}' has no field named '{}'",
                    parent_type.type_name(db),
                    field.as_str(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &field.get_span(db)).unwrap_or_default())
                .call(),
            Self::AccessDeclTypeMismatch {
                var_origin,
                spec,
                expected,
                actual,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "access declaration expects '{}', but variable has type '{}'",
                        expected.type_name(db),
                        actual.type_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &spec.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "variable '{}' is declared here",
                        var_origin.name_with_case(db).text(db),
                    ),
                    var_origin.get_scope_id(db).file(db),
                    var_origin.get_name_span(db),
                ));

                diag
            }
            Self::ProgElementRefused { expr, why } => {
                let (message, note, help) = match why {
                    ProgElementRefusal::NotAnInput { var } => (
                        format!(
                            "'{}' is not a VAR_INPUT of the program, so ':=' cannot feed it",
                            var.text(db)
                        ),
                        Some("':=' connects a source to an input, '=>' an output to a sink"),
                        None,
                    ),
                    ProgElementRefusal::NotAnOutput { var } => (
                        format!(
                            "'{}' is not a VAR_OUTPUT of the program, so '=>' cannot read it",
                            var.text(db)
                        ),
                        Some("':=' connects a source to an input, '=>' an output to a sink"),
                        None,
                    ),
                    ProgElementRefusal::TypeMismatch {
                        var,
                        declared,
                        global,
                        ty,
                    } => (
                        format!(
                            "'{}' is '{}', while '{}' is '{}'",
                            var.text(db),
                            declared.type_name(db),
                            global.text(db),
                            ty.type_name(db)
                        ),
                        Some("a connection copies the value as it is"),
                        Some("connect a variable of the same type"),
                    ),
                    ProgElementRefusal::WidthMismatch {
                        var,
                        declared,
                        address,
                        bits,
                    } => (
                        format!(
                            "'{}' is '{}', while '{address}' is {bits} bit{}",
                            var.text(db),
                            declared.type_name(db),
                            if *bits == 1 { "" } else { "s" }
                        ),
                        Some("an address connects to a variable as wide as it is"),
                        None,
                    ),
                    ProgElementRefusal::NoSuchGlobal { name } => (
                        format!("no VAR_GLOBAL is named '{}'", name.text(db)),
                        Some(
                            "a connection names a VAR_GLOBAL of the configuration, an address, or a constant",
                        ),
                        None,
                    ),
                    ProgElementRefusal::ConnectedTwice { var } => (
                        format!(
                            "'{}' is connected here and by another element",
                            var.text(db)
                        ),
                        Some("an input has one source"),
                        Some("keep one of the elements"),
                    ),
                    ProgElementRefusal::NotAVariable => (
                        "an element names a variable by its name alone".to_string(),
                        None,
                        Some(
                            "connect the program's inputs and outputs to VAR_GLOBALs, addresses or constants, and associate the function blocks it holds",
                        ),
                    ),
                    ProgElementRefusal::NotAFunctionBlock { var } => (
                        format!(
                            "'{}' is not a FUNCTION_BLOCK instance, so no task can run it",
                            var.text(db)
                        ),
                        Some("a CLASS has no body for a task to run"),
                        None,
                    ),
                    ProgElementRefusal::AssociatedTwice { var } => (
                        format!(
                            "'{}' is associated with a task here and by another element",
                            var.text(db)
                        ),
                        Some("a function block runs under one task"),
                        Some("keep one of the elements"),
                    ),
                    ProgElementRefusal::UnknownTask { task } => (
                        format!("no TASK is named '{}' in this resource", task.text(db)),
                        None,
                        Some("associate the function block with a TASK the resource declares"),
                    ),
                    ProgElementRefusal::UnboundConformand { var, param } => (
                        format!(
                            "no task binds the ARRAY[*] '{}' of '{}'",
                            param.text(db),
                            var.text(db)
                        ),
                        Some("an ARRAY[*] VAR_IN_OUT has the bounds of what a call binds to it"),
                        Some("call the function block from the program's body"),
                    ),
                    ProgElementRefusal::CalledByProgram { var, program, .. } => (
                        format!(
                            "'{}' is run by its task and called by '{}' too",
                            var.text(db),
                            program.text(db)
                        ),
                        Some("the task runs the instance on its own"),
                        Some("remove the call from the program"),
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                if let ProgElementRefusal::CalledByProgram { var, call, .. } = why {
                    diag.with_related(ide_diagnostic::Related::new(
                        format!("'{}' is called here", var.text(db)),
                        call.get_scope_id(db).file(db),
                        call.get_span(db),
                    ));
                }
                diag.with_advice(note, help);
                diag
            }
            Self::DirectVariableUnsupported { site, address, why } => {
                let mut diag = diag()
                    .message(why.message(address))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                let (note, help) = why.advice();
                diag.with_advice(note, help);
                diag
            }
            Self::UnknownMultibitsAccess { expr, access } => diag()
                .message(format!(
                    "'%{access}' names no access size (expected X, B, W, D or L)"
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::MultibitsOutOfRange {
                expr,
                var,
                offset,
                access_bits,
                max_offset,
                base_type,
            } => {
                let message = match max_offset {
                    Some(max) => format!(
                        "offset {} is out of range for type '{}' (valid range: 0..{})",
                        offset,
                        base_type.type_name(db),
                        max,
                    ),
                    None => format!(
                        "a {}-bit access does not fit in type '{}'",
                        access_bits,
                        base_type.type_name(db),
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                if let Some(var) = var {
                    diag.with_related(Related::new(
                        format!("'{}' is declared here", var.name_with_case(db).text(db)),
                        var.scope_id(db).file(db),
                        var.get_name_span(db),
                    ));
                }

                diag
            }
            Self::PartialAccessWithoutParts {
                expr,
                var,
                base_type,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "type '{}' has no parts to access",
                        base_type.type_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "a partial access applies to a bit string, an integer or a BOOL".to_string(),
                );
                // A REAL's bits encode the number: the conversion to the bit
                // string of its width is what says the encoding is wanted.
                let normalized = base_type.normalize(db);
                if let Type::RefTo(target) = normalized
                    && crate::hir_ty::infer::Infer::infer(&target, db)
                        .normalize(db)
                        .takes_partial_access()
                {
                    diag.with_help(
                        "dereference it with '^' and access the parts of what it points to"
                            .to_string(),
                    );
                }
                if let Type::Elementary(spec) = normalized {
                    use crate::hir_def::expressions::spec::ElementarySpec as E;
                    let bits = match spec {
                        E::Real => Some(E::DWord),
                        E::LReal => Some(E::LWord),
                        E::Char => Some(E::Byte),
                        _ => None,
                    };
                    if let Some(bits) = bits.filter(|bits| bits.explicit_cast(spec)) {
                        diag.with_help(format!(
                            "convert it with '{}_TO_{}' and access the parts of the '{}'",
                            spec.type_name(),
                            bits.type_name(),
                            bits.type_name(),
                        ));
                    }
                }
                if let Some(var) = var {
                    diag.with_related(Related::new(
                        format!("'{}' is declared here", var.name_with_case(db).text(db)),
                        var.scope_id(db).file(db),
                        var.get_name_span(db),
                    ));
                }
                diag
            }
            Self::FrameLargerThanStack {
                site,
                what,
                frame,
                stack,
            } => {
                let mut diag = diag()
                    .message(format!("{what} pushes {frame} bytes"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(format!(
                    "a recursive call pushes its frame on the stack, and `stack_size` in config.toml makes it {stack} bytes"
                ));
                diag.with_help(format!("make `stack_size` {frame} bytes or more"));
                diag
            }
            Self::WriteToInputLocation { site, address, via } => {
                let message = match via {
                    InputWriteRoute::Assignment => format!(
                        "'{address}' is an input: it is written by the host, not by the program"
                    ),
                    InputWriteRoute::Reference => format!(
                        "'{address}' is an input, so a writable reference to it cannot be taken"
                    ),
                    InputWriteRoute::Initializer => format!(
                        "'{address}' is an input, so an initial value is overwritten before anything reads it"
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    match via {
                        InputWriteRoute::Assignment => {
                            "the host copies the input image in before each scan, so this write is overwritten before anything can read it"
                        }
                        InputWriteRoute::Reference => {
                            "a REF_TO is a writable pointer and nothing tracks what is stored through it, so the reference is refused where it is taken"
                        }
                        InputWriteRoute::Initializer => {
                            "the host writes the input image before every scan, the first one included"
                        }
                    }
                    .to_string(),
                );
                diag
            }
            Self::RetainOnIoLocation { var, address } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is located at '{address}' and cannot be RETAIN",
                        var.get_name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                if address.ends_with('*') {
                    diag.with_note("a variable VAR_CONFIG locates points at its channel and has no storage of its own to retain".to_string());
                    diag.with_help("to persist a marker, declare it located in full, RETAIN, in a PROGRAM or as a VAR_GLOBAL".to_string());
                } else {
                    diag.with_note("the retain band is restored at startup, so a retained I/O image would run the first scan on the values of the last power cycle".to_string());
                }
                diag
            }
            Self::DuplicateLocation {
                var,
                other,
                address,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' and '{}' are both located at '{address}'",
                        var.get_name_with_case(db).text(db),
                        other.get_name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &location_span(db, var)).unwrap_or_default(),
                    )
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "'{}' is located here",
                        other.get_name_with_case(db).text(db)
                    ),
                    other.get_scope_id(db).file(db),
                    location_span(db, other),
                ));
                diag.with_note(
                    "each declaration gets storage of its own, so the two would never see each other's value".to_string(),
                );
                diag.with_help("declare the address once and name that variable".to_string());
                diag
            }
            Self::LocationWidthMismatch {
                var,
                address,
                address_bits,
                declared_bits,
                declared,
            } => {
                let wide = |n: usize| match n {
                    1 => "1 bit".to_string(),
                    n => format!("{n} bits"),
                };
                let bits = wide(*address_bits);
                let name = var.get_name_with_case(db).text(db);
                let message = match declared_bits {
                    Some(declared_bits) => format!(
                        "'{name}' is '{}' ({}), while '{address}' is {bits}",
                        declared.type_name(db),
                        wide(*declared_bits)
                    ),
                    None => format!(
                        "'{name}' is '{}', not an elementary type, while '{address}' is {bits}",
                        declared.type_name(db)
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var.spec(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "a located variable holds one value as wide as its address".to_string(),
                );
                diag.with_help(format!(
                    "declare it as an elementary type of {bits}, such as {}",
                    match address_bits {
                        1 => "BOOL",
                        8 => "BYTE, SINT or USINT",
                        16 => "WORD, INT or UINT",
                        32 => "DWORD, DINT or REAL",
                        _ => "LWORD, LINT or LREAL",
                    }
                ));
                diag
            }
            Self::PartOfWiderAddress {
                site,
                address,
                owner,
                usage,
            } => {
                let mut diag = diag()
                    .message(usage.message(address, owner))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                let (note, help) = usage.advice(owner);
                diag.with_advice(note, help);
                diag
            }
            Self::ConfigLocationRefused {
                expr,
                var,
                address,
                why,
            } => {
                let mut diag = diag()
                    .message(why.message(db, var.text(db), address))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                let (note, help) = why.advice();
                diag.with_advice(note, help);
                diag
            }
            Self::ConfigEntryRefused { expr, why } => {
                let (message, note, help) = match why {
                    ConfigEntryRefusal::PathStep => (
                        "a VAR_CONFIG path names instances and variables, not an element or what a reference points at".to_string(),
                        None,
                        Some("name the variable itself"),
                    ),
                    ConfigEntryRefusal::TypeMismatch { var, written, declared } => (
                        format!(
                            "the entry says '{}', but '{}' is declared '{}'",
                            written.type_name(db),
                            var.text(db),
                            declared.type_name(db)
                        ),
                        Some("the entry repeats the variable's type"),
                        Some("write the declared type"),
                    ),
                    ConfigEntryRefusal::ValueTwice { var } => (
                        format!(
                            "'{}' is given a value here and by another entry",
                            var.text(db)
                        ),
                        Some("an instance's variable has one starting value"),
                        Some("keep one of the entries"),
                    ),
                    ConfigEntryRefusal::ChannelDeclared { var, address } => (
                        format!(
                            "'{}' is at '{address}', where a declaration gives its starting value",
                            var.text(db)
                        ),
                        None,
                        Some("give the value in that declaration"),
                    ),
                    ConfigEntryRefusal::ProgramValue => (
                        "a VAR_CONFIG value is given to a variable of an instance, not to the PROGRAM instance itself".to_string(),
                        None,
                        Some("give each variable its value in an entry of its own"),
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_advice(note, help);
                diag
            }
            Self::PartlyLocatedUnlocated(PartlyUnlocated::Missing {
                instance,
                path,
                address,
            }) => {
                let path = dotted(
                    db,
                    std::iter::once(instance.with_case).chain(path.iter().copied()),
                );
                let mut diag = diag()
                    .message(format!(
                        "no VAR_CONFIG entry locates '{path}', declared AT {address}"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &instance.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_help(
                    "give each instance its address in VAR_CONFIG, as in 'Res.P1.fb.x AT %IX0.0 : BOOL;'".to_string(),
                );
                diag
            }
            Self::PartlyLocatedUnlocated(PartlyUnlocated::Returned {
                ret,
                callable,
                ty,
                member,
                address,
            }) => {
                let (ty, member) = (ty.type_name(db), dotted(db, member.iter().copied()));
                let mut diag = diag()
                    .message(format!(
                        "the {callable} returns a '{ty}' holding '{member}', declared AT {address}"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &ret.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "a VAR_CONFIG path names a PROGRAM instance and the instances it holds by name, not a result made for each call".to_string(),
                );
                diag.with_help(format!(
                    "hold the instance in a PROGRAM and pass it to the {callable} as a VAR_IN_OUT"
                ));
                diag
            }
            Self::PartlyLocatedUnlocated(PartlyUnlocated::Unreachable {
                var,
                member,
                address,
                place,
            }) => {
                let whose = format!(
                    "'{}' holds '{}', declared AT {address}",
                    var.get_name_with_case(db).text(db),
                    dotted(db, member.iter().copied())
                );
                let message = match place {
                    UnreachablePlace::Array => format!("{whose}, in the elements of an array"),
                    UnreachablePlace::Global => format!("{whose}, in a VAR_GLOBAL"),
                    UnreachablePlace::PerCall => {
                        format!("{whose}, in an instance made for each call")
                    }
                    UnreachablePlace::Struct => format!("{whose}, in a field of a STRUCT"),
                    UnreachablePlace::Input => format!("{whose}, in a VAR_INPUT"),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                match place {
                    UnreachablePlace::Input => {
                        diag.with_note(
                            "each call overwrites a VAR_INPUT with a copy of its argument"
                                .to_string(),
                        );
                        diag.with_help("pass the instance as a VAR_IN_OUT".to_string());
                    }
                    _ => {
                        diag.with_note(
                            "a VAR_CONFIG path names a PROGRAM instance and the instances it holds by name".to_string(),
                        );
                        diag.with_help(
                            "hold the instance in a PROGRAM, or in a function block the PROGRAM holds".to_string(),
                        );
                    }
                }
                diag
            }
            Self::RetainHoldsPartlyLocated { var, member } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is RETAIN and holds '{}', declared AT %M*, with no storage of its own to retain",
                        var.get_name_with_case(db).text(db),
                        dotted(db, member.iter().copied())
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var.get_name_span(db))
                            .unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "a variable VAR_CONFIG locates points at its marker and has no storage of its own to retain".to_string(),
                );
                diag.with_help(
                    "to persist a marker, declare it located in full, RETAIN, in a PROGRAM or as a VAR_GLOBAL".to_string(),
                );
                diag
            }
            Self::PartlyLocatedOverwritten {
                site,
                member,
                address,
            } => {
                let member = dotted(db, member.iter().copied());
                let mut diag = diag()
                    .message(format!(
                        "'{member}' is declared AT {address} and has no value of its own to initialize"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note("it points at the channel VAR_CONFIG gives it".to_string());
                diag.with_help("give it its starting value in its VAR_CONFIG entry".to_string());
                diag
            }
            Self::PartlyLocatedCopied {
                site,
                member,
                address,
            } => {
                let member = dotted(db, member.iter().copied());
                let mut diag = diag()
                    .message(format!(
                        "the copy writes over '{member}', declared AT {address}"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "it points at the channel VAR_CONFIG gives its instance, and the copy would point it at the other instance's"
                        .to_string(),
                );
                diag.with_help(
                    "copy the other members one by one, or share the instance through a VAR_IN_OUT"
                        .to_string(),
                );
                diag
            }
        }
    }
}

/// The address a declaration is located at, or its name when it has none.
fn location_span<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: &VariableDecl<'db>,
) -> tree_sitter::Range {
    match var.location(db) {
        Some(location) => location.get_span(db),
        None => var.get_name_span(db),
    }
}

/// A member path as written: `fb.x`.
fn dotted(db: &dyn WorkspaceDataBase, idents: impl Iterator<Item = Ident>) -> String {
    idents
        .map(|ident| ident.text(db).as_str())
        .collect::<Vec<_>>()
        .join(".")
}
