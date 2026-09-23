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
    /// A configuration construct that is parsed but does nothing. Reported so a
    /// user is not left believing state they wrote is being applied — the
    /// silent version is worse than a rejection, because the compiler accepts
    /// the input and then ignores it.
    UnsupportedConfigElement {
        expr: PathExpr<'db>,
        kind: UnsupportedConfigKind,
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
        var: compact_str::CompactString,
        /// The address as written in the entry.
        address: compact_str::CompactString,
        why: ConfigLocationRefusal,
    },
    /// A variable declared `AT %I*`, `%Q*` or `%M*` that VAR_CONFIG does not
    /// locate, or cannot.
    PartlyLocatedUnlocated(PartlyUnlocated<'db>),
    /// A VAR_CONFIG entry that resolves but cannot be taken as written.
    ConfigEntryRefused {
        expr: PathExpr<'db>,
        why: ConfigEntryRefusal,
    },
}

/// Why a VAR_CONFIG entry cannot be taken as written.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ConfigEntryRefusal {
    /// `Res.P1.arr[0]`, `Res.P1.p^.x`: a path names instances and variables.
    PathStep,
    /// The type the entry repeats is not the variable's.
    TypeMismatch {
        var: compact_str::CompactString,
        written: compact_str::CompactString,
        declared: compact_str::CompactString,
    },
}

/// Why a VAR_CONFIG entry cannot give the variable it names this address.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ConfigLocationRefusal {
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
        declared: compact_str::CompactString,
    },
    /// A bit inside a wider address the workspace names, which has no address
    /// of its own for the variable to point at.
    BitOfWider { owner: compact_str::CompactString },
    /// Another entry locates the same instance's variable.
    LocatedTwice { other: compact_str::CompactString },
}

impl ConfigLocationRefusal {
    fn message(&self, var: &str, address: &str) -> String {
        match self {
            Self::NotPartlyLocated => format!(
                "'{var}' is not declared AT %I*, %Q* or %M*, so its address is not VAR_CONFIG's to give"
            ),
            Self::AreaMismatch { declared } => {
                format!("'{var}' is declared AT {declared}, and '{address}' is not in that area")
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
                match declared_bits {
                    Some(n) => format!(
                        "'{address}' is {}, but '{var}' is declared '{declared}', which is {n}",
                        bits(*address_bits)
                    ),
                    None => format!(
                        "'{address}' is {}, but '{var}' is declared '{declared}', which has no width of its own",
                        bits(*address_bits)
                    ),
                }
            }
            Self::BitOfWider { owner } => format!(
                "'{address}' is a bit of '{owner}', and a bit has no address to locate '{var}' at"
            ),
            Self::LocatedTwice { other } => {
                format!("'{var}' is located at '{address}' here and at '{other}' by another entry")
            }
        }
    }

    fn note(&self) -> &'static str {
        match self {
            Self::NotPartlyLocated => {
                "declare it AT %I*, %Q* or %M* in its POU to leave its address to the configuration"
            }
            Self::AreaMismatch { .. } => {
                "the area is the declaration's: give an input an address in %I, an output one in %Q, a marker one in %M"
            }
            Self::Unlocatable { .. } => {
                "VAR_CONFIG gives the complete address, such as '%IX0.0' or '%QW4'"
            }
            Self::Width { .. } => {
                "the variable holds one value as wide as its address; give it an address of its type's width"
            }
            Self::BitOfWider { .. } => {
                "the variable points at its channel, so give it a byte or wider, or a bit nothing wider around it is named"
            }
            Self::LocatedTwice { .. } => {
                "an instance's variable has one address; keep one of the entries"
            }
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
        /// The path from the instance: `P1.fb.x`.
        path: compact_str::CompactString,
        /// `%I*`, `%Q*` or `%M*`.
        address: compact_str::CompactString,
    },
    /// Instances held where no VAR_CONFIG path reaches them.
    Unreachable {
        var: VariableDecl<'db>,
        /// The member declared with the partial address: `x`, or `fb.x`.
        member: compact_str::CompactString,
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
}

/// What a part of a wider address was used for that it cannot be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum WiderAddressUse {
    /// Passed to a VAR_IN_OUT, which takes an address.
    InOut,
    /// Given to `REF()`, which takes an address.
    Reference,
    /// Used as a FOR counter, which needs storage of its own.
    ForCounter,
    /// Declared RETAIN: persistence belongs to storage, and this has none.
    Retain,
    /// Given an initial value: `__init` writes storage, and this has none,
    /// so the value was silently dropped.
    Initializer,
}

impl WiderAddressUse {
    fn message(&self, address: &str, owner: &str) -> String {
        let is = format!("'{address}' is part of '{owner}'");
        match self {
            Self::InOut => format!("{is} and has no address of its own to pass to a VAR_IN_OUT"),
            Self::Reference => format!("{is} and has no address of its own to take a reference to"),
            Self::ForCounter => format!("{is} and cannot count a FOR loop"),
            Self::Retain => format!("{is} and cannot be RETAIN on its own"),
            Self::Initializer => format!("{is} and cannot have an initial value of its own"),
        }
    }

    fn note(&self, address: &str, owner: &str) -> String {
        match self {
            Self::InOut => "copy it into a variable, pass that, and assign it back".to_string(),
            Self::Reference => format!("take the reference of '{owner}' as a whole"),
            Self::ForCounter => format!("count in a variable and assign '{address}' from it"),
            Self::Retain => {
                format!(
                    "RETAIN belongs on the variable located at '{owner}', whose storage this is"
                )
            }
            Self::Initializer => format!(
                "give the variable located at '{owner}' an initial value with this part set in it"
            ),
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
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::InPou => {
                "a function's, function block's or class's variables belong to each call or instance, so one address cannot be theirs; declare it in a PROGRAM, or as a VAR_GLOBAL of the CONFIGURATION, and name it from here"
            }
            Self::Incomplete => {
                "VAR_CONFIG completes a partial address for a variable of a PROGRAM, FUNCTION_BLOCK or CLASS, instance by instance; anywhere else, write the address in full"
            }
            Self::Malformed => {
                "an address names its area with I, Q or M and its width with X, B, W, D or L, as in '%IX0.0'; a bit may leave the width out, as in '%I0.0'"
            }
            Self::NotAreaOnly => {
                "the variable's type gives the width, and VAR_CONFIG gives the rest of the address"
            }
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

/// Which parsed-but-inert CONFIGURATION construct was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum UnsupportedConfigKind {
    /// `PROGRAM P WITH T : Type (in := src)` — the connection list is read and
    /// then discarded: no copy is emitted around the scan, and the names are
    /// never resolved, so a typo passes silently.
    ProgramConnection,
    /// `PROGRAM P WITH T : Type (fb WITH other_task)` — associating a nested
    /// FB with its own task.
    FbTaskAssociation,
    /// `VAR_CONFIG PA.x : INT := 42;` — resolved and type-checked against the
    /// instance's field, then thrown away, so the field keeps its declared
    /// value. The validation makes this one especially misleading.
    InstanceInit,
    /// The same, on a variable declared `AT %I*`, which the grammar gives no
    /// initial value of its own to fall back on.
    InstanceInitLocated,
}

impl UnsupportedConfigKind {
    fn message(self) -> &'static str {
        match self {
            Self::ProgramConnection => {
                "program connection lists are parsed but not wired up yet, so this has no effect"
            }
            Self::FbTaskAssociation => {
                "associating a function block with its own task is not supported yet"
            }
            Self::InstanceInit | Self::InstanceInitLocated => {
                "a VAR_CONFIG value is checked but not applied yet, so it never reaches the instance"
            }
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::ProgramConnection => "assign it in the program body instead",
            Self::FbTaskAssociation => "run the function block from its enclosing program's task",
            Self::InstanceInit => "set the value in the variable's own declaration instead",
            Self::InstanceInitLocated => {
                "a variable VAR_CONFIG locates starts at its type's default, or at the value its channel's own declaration gives it"
            }
        }
    }
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
            Self::EventDriven => {
                "event-driven tasks (SINGLE) are not supported yet; only cyclic tasks run"
            }
            Self::NoTrigger => "a TASK needs an INTERVAL to run its programs",
            Self::NonLiteralInterval => {
                "INTERVAL must be a TIME literal or a CONSTANT global holding one"
            }
            Self::ZeroInterval => "INTERVAL must be greater than zero",
        }
    }

    /// The follow-up a user needs to actually fix it. The message says what is
    /// wrong; this says what to write instead.
    fn note(self) -> Option<&'static str> {
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
            Self::UnsupportedConfigElement { .. } => "E1416",
            Self::DirectVariableUnsupported { .. } => "E1417",
            Self::UnknownMultibitsAccess { .. } => "E1418",
            Self::WriteToInputLocation { .. } => "E1419",
            Self::RetainOnIoLocation { .. } => "E1420",
            Self::DuplicateLocation { .. } => "E1421",
            Self::LocationWidthMismatch { .. } => "E1422",
            Self::PartOfWiderAddress { .. } => "E1423",
            Self::ConfigLocationRefused { .. } => "E1424",
            Self::PartlyLocatedUnlocated(_) => "E1425",
            Self::ConfigEntryRefused { .. } => "E1426",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::NoConfigFileFound { .. } => "configuration error",
            Self::MultipleConfigurations { .. } => "configuration error",
            Self::MultipleResources { .. } => "configuration error",
            Self::TaskOrProgramOutsideResource(_) => "syntax",
            Self::MissingPriority(_) => "syntax",
            Self::InvalidPriority { .. } => "configuration error",
            Self::IntervalAfterPriority(_) => "syntax",
            Self::SingleAfterInterval(_) => "syntax",
            Self::SingleAfterPriority(_) => "syntax",
            Self::UnschedulableTask { .. } => "task cannot be scheduled",
            Self::UnknownTaskRef { .. } => "configuration error",
            Self::ProgramWithoutTask { .. } => "program instance never runs",
            Self::ConfigInstInitUnknownInstance { .. } => "configuration error",
            Self::ConfigInstInitFieldNotFound { .. } => "configuration error",
            Self::AccessDeclTypeMismatch { .. } => "access declaration type mismatch",
            Self::UnsupportedConfigElement { .. } => "unsupported configuration element",
            Self::DirectVariableUnsupported { .. } => "address cannot be located",
            Self::UnknownMultibitsAccess { .. } => "unknown multibit access size",
            Self::WriteToInputLocation { .. } => "write to an input location",
            Self::RetainOnIoLocation { .. } => "RETAIN on an I/O location",
            Self::DuplicateLocation { .. } => "duplicate location",
            Self::LocationWidthMismatch { .. } => "location type mismatch",
            Self::PartOfWiderAddress { .. } => "part of a wider address",
            Self::ConfigLocationRefused { .. } => "location refused",
            Self::PartlyLocatedUnlocated(_) => "variable not located",
            Self::ConfigEntryRefused { .. } => "configuration entry refused",
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
                    .message("no configuration file found".to_string())
                    .severity(DiagnosticSeverity::HINT)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &file.document(db).tree.root_node().range())
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_note(format!(
                    "a configuration file is required at the root of your workspace (inside '{}')",
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
                        "a workspace can only have one CONFIGURATION; this one declares {}",
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
                        format!("'{}' is declared here", other.name(db).text(db)),
                        other.get_scope_id(db).file(db),
                        other.get_name_span(db),
                    ));
                }
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
                diag()
                    .message(format!(
                        "a deployment drives one RESOURCE; this configuration declares {} ({list}); deploy one RESOURCE per runtime",
                        names.len(),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call()
            }
            Self::TaskOrProgramOutsideResource(span) => {
                let mut diag = diag()
                    .message("TASK and PROGRAM must be declared inside a RESOURCE".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                diag.with_note(
                    "wrap them in a RESOURCE <name> ON <cpu> ... END_RESOURCE block".into(),
                );
                diag
            }
            Self::MissingPriority(span) => diag()
                .message("PRIORITY is required in TASK configuration".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::InvalidPriority { task, value } => {
                let mut diag = diag()
                    .message(format!(
                        "task '{}' has an unusable PRIORITY '{}'",
                        task.ident.text(db),
                        value.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &task.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "PRIORITY must fit in a 32-bit unsigned integer; 0 is the most urgent".into(),
                );
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
                        task.ident.text(db),
                        reason.message()
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &task.get_span(db)).unwrap_or_default())
                    .call();
                if let Some(note) = reason.note() {
                    diag.with_note(note.to_string());
                }
                diag
            }
            Self::UnknownTaskRef { task } => diag()
                .message(format!(
                    "task '{}' not found in this configuration",
                    task.ident.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &task.get_span(db)).unwrap_or_default())
                .call(),
            Self::ProgramWithoutTask { instance } => diag()
                .message(format!(
                    "program instance '{}' has no WITH <task>, so it will never run",
                    instance.ident.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &instance.get_span(db)).unwrap_or_default())
                .call(),
            Self::ConfigInstInitUnknownInstance { instance_name } => diag()
                .message(format!(
                    "no program instance '{}' found in this configuration",
                    instance_name.text(db)
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
                    field.text(db)
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
                        var_origin.name(db).text(db),
                    ),
                    var_origin.get_scope_id(db).file(db),
                    var_origin.get_name_span(db),
                ));

                diag
            }
            Self::UnsupportedConfigElement { expr, kind } => {
                let mut diag = diag()
                    .message(kind.message().to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(kind.note().to_string());
                diag
            }
            Self::DirectVariableUnsupported { site, address, why } => {
                let mut diag = diag()
                    .message(why.message(address))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(why.note().to_string());
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
                        var.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var.as_call_site(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();
                diag.with_note(if address.ends_with('*') {
                    "a variable VAR_CONFIG locates points at its channel and has no storage of its own to retain; to persist a marker, declare it located in full, RETAIN, in a PROGRAM or as a VAR_GLOBAL".to_string()
                } else {
                    "the retain band is restored at startup, so a retained I/O image would run the first scan on the values of the last power cycle; only '%M' may persist".to_string()
                });
                diag
            }
            Self::DuplicateLocation {
                var,
                other,
                address,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is located at '{address}', which '{}' also claims",
                        var.get_name_ident(db).text(db),
                        other.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var.as_call_site(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();
                diag.with_related(Related::new(
                    format!("'{}' is located here", other.get_name_ident(db).text(db)),
                    other.get_scope_id(db).file(db),
                    other.as_call_site(db).get_span(db),
                ));
                diag.with_note(
                    "an address is one channel, and each declaration is given storage of its own, so the two would never see each other's value; name the one variable from wherever it is needed".to_string(),
                );
                diag
            }
            Self::LocationWidthMismatch {
                var,
                address,
                address_bits,
                declared_bits,
                declared,
            } => {
                let bits = format!(
                    "{address_bits} bit{}",
                    if *address_bits == 1 { "" } else { "s" }
                );
                let name = var.get_name_ident(db).text(db);
                let message = match declared_bits {
                    Some(declared_bits) => format!(
                        "'{address}' is {bits}, but '{name}' is declared '{}', which is {declared_bits}",
                        declared.type_name(db)
                    ),
                    None => format!(
                        "'{address}' is {bits}, but '{name}' is declared '{}', which is not an elementary type of any width",
                        declared.type_name(db)
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var.as_call_site(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();
                diag.with_note(format!(
                    "a located variable holds one value as wide as its address; declare it as an elementary type of {bits}, such as {}",
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
                diag.with_note(usage.note(address, owner));
                diag
            }
            Self::ConfigLocationRefused {
                expr,
                var,
                address,
                why,
            } => {
                let mut diag = diag()
                    .message(why.message(var, address))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(why.note().to_string());
                diag
            }
            Self::ConfigEntryRefused { expr, why } => {
                let (message, note) = match why {
                    ConfigEntryRefusal::PathStep => (
                        "a VAR_CONFIG path names instances and variables, not an element or what a reference points at".to_string(),
                        "name the variable itself; an element of an array or a referenced value cannot be configured",
                    ),
                    ConfigEntryRefusal::TypeMismatch { var, written, declared } => (
                        format!("the entry says '{written}', but '{var}' is declared '{declared}'"),
                        "the entry repeats the variable's type; write the declared one",
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(note.to_string());
                diag
            }
            Self::PartlyLocatedUnlocated(PartlyUnlocated::Missing {
                instance,
                path,
                address,
            }) => {
                let mut diag = diag()
                    .message(format!(
                        "'{path}' is declared AT {address}, and no VAR_CONFIG entry locates it"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &instance.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "each instance is given its address in the CONFIGURATION's VAR_CONFIG, as in 'Res.P1.fb.x AT %IX0.0 : BOOL;'".to_string(),
                );
                diag
            }
            Self::PartlyLocatedUnlocated(PartlyUnlocated::Unreachable {
                var,
                member,
                address,
                place,
            }) => {
                let whose = format!(
                    "'{}' holds '{member}', declared AT {address}",
                    var.get_name_ident(db).text(db)
                );
                let message = match place {
                    UnreachablePlace::Array => format!(
                        "{whose}, in the elements of an array, which VAR_CONFIG cannot name"
                    ),
                    UnreachablePlace::Global => {
                        format!("{whose}, in a VAR_GLOBAL, which VAR_CONFIG cannot name")
                    }
                    UnreachablePlace::PerCall => format!(
                        "{whose}, in an instance made for each call, which VAR_CONFIG cannot name"
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var.as_call_site(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "a VAR_CONFIG path names a PROGRAM instance and the instances it holds by name; hold this one there".to_string(),
                );
                diag
            }
        }
    }
}
