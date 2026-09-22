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
}

/// Why a TASK cannot be scheduled. Only cyclic tasks with a literal, non-zero
/// INTERVAL are; each other shape gets its own message rather than a shared
/// "unsupported", because the fix differs in every case.
/// Why an address has no storage to be given. Each has its own fix, so each
/// says its own: one message covering all three said only that the address
/// was unsupported, which was true of none of them once the bands landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum UnlocatableAddress {
    /// An `AT` clause on a POU's own variable.
    InPou,
    /// `%I*`: the address is deliberately incomplete.
    Incomplete,
    /// No area letter, or no width letter — `%I1` is Table 16 row 4b, where
    /// the size character is omitted and means BOOL.
    Malformed,
}

impl UnlocatableAddress {
    fn message(self, address: &str) -> String {
        match self {
            Self::InPou => format!("'{address}' cannot locate a variable of a POU"),
            Self::Incomplete => format!("'{address}' is not a complete address"),
            Self::Malformed => format!("'{address}' does not name an area and a width"),
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::InPou => {
                "a POU's variables are fields of its instance, which is laid out as one unit, so a field cannot also sit in a band the host copies whole; declare it as a VAR_GLOBAL of the CONFIGURATION and name it from the POU"
            }
            Self::Incomplete => {
                "the binding for a partly specified address comes from VAR_CONFIG, which is checked but not applied yet (E1416); write the address in full to allocate it now"
            }
            Self::Malformed => {
                "an address names its area with I, Q or M and its width with X, B, W, D or L, as in '%IX0.0'; omitting the size character is not implemented"
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
            Self::InstanceInit => {
                "VAR_CONFIG is checked but not applied yet, so this value never reaches the instance"
            }
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::ProgramConnection => "assign it in the program body instead",
            Self::FbTaskAssociation => "run the function block from its enclosing program's task",
            Self::InstanceInit => "set the value in the program's own VAR declaration instead",
        }
    }
}

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
                diag.with_note(
                    "the retain band is restored at startup, so a retained I/O image would run the first scan on the values of the last power cycle; only '%M' may persist"
                        .to_string(),
                );
                diag
            }
        }
    }
}
