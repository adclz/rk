use db::WorkspaceDataBase;
use ide_diagnostic::{IdeDiagnostic, Related};

use crate::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Elementary, Expr, ExprKind, InitExpr, InitExprKind, Integer,
                IntegerKind, PrimaryExpr, RefValue,
            },
            invocation::InvocationKind,
            spec::{ElementarySpec, Spec, SpecKind},
        },
        scope::ScopeKind,
        semantic_index::semantic_index,
    },
    hir_ty::{
        infer::Infer,
        ty::{CallableType, InferType, Type},
    },
};

/// Returns the display name for a spec, handling sized strings specially.
fn spec_type_name<'db>(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> String {
    match spec.kind(db) {
        SpecKind::SizedString(length) => {
            let len = length
                .as_range(db)
                .map(|n| n.to_string())
                .unwrap_or_default();
            format!("STRING[{}]", len)
        }
        _ => spec.infer(db).type_name(db),
    }
}

impl ElementarySpec {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool => "BOOL",
            Self::REDGEBool => "BOOL (R_EDGE)",
            Self::FEDGEBool => "BOOL (F_EDGE)",
            Self::Byte => "BYTE",
            Self::Word => "WORD",
            Self::DWord => "DWORD",
            Self::LWord => "LWORD",
            Self::SInt => "SINT",
            Self::USInt => "USINT",
            Self::UInt => "UINT",
            Self::Int => "INT",
            Self::DInt => "DINT",
            Self::UDInt => "UDINT",
            Self::LInt => "LINT",
            Self::ULInt => "ULINT",
            Self::Real => "REAL",
            Self::LReal => "LREAL",
            Self::String => "STRING",
            Self::Char => "CHAR",
            Self::Date => "DATE",
            Self::LDate => "LDATE",
            Self::DateAndTime => "DT",
            Self::LDateTime => "LDT",
            Self::Time => "TIME",
            Self::LTime => "LTIME",
            Self::Tod => "TOD",
            Self::LTod => "LTOD",
        }
    }
}

impl<'db> Type<'db> {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Program(program) => "PROGRAM",
            Self::Config(_) => "CONFIGURATION",
            Self::Resource(_) => "RESOURCE",
            Self::Task(_) => "TASK",
            Self::Elementary(_) => "ELEMENTARY",
            Self::Array(arr) => "ARRAY",
            Self::ArrayConformand(_) => "ARRAY_CONFORMAND",
            Self::Class(c) => "CLASS",
            Self::DataType(typ) => "DATATYPE",
            Self::Function(f) => "FUNCTION",
            Self::FunctionBlock(fb) => "FUNCTION_BLOCK",
            Self::Interface(i) => "INTERFACE",
            Self::MethodDecl(m) => "METHOD",
            Self::SubRange(_) => "SUBRANGE",
            Self::DirectVariable(_) => "DIRECT_VARIABLE",
            Self::Variable(_) => "VARIABLE",
            Self::Struct(s) => "STRUCT",
            Self::StructElement(_) => "STRUCT_ELEMENT",
            Self::Enum(e) => "ENUM",
            Self::EnumVariant(_) => "ENUM_VARIANT",
            Self::CallableType(_) => "CALLABLE",
            Self::RefTo(_) => "REF_TO",
            Self::Null => "NULL",
            Self::Infer(_) => "INFER",
            Self::Void => "VOID",
            Self::Never => "NEVER",
        }
    }

    pub fn type_name(&self, db: &'db dyn WorkspaceDataBase) -> String {
        match self {
            Self::Elementary(elem) => elem.type_name().into(),
            Self::Program(program) => program.get_name_ident(db).text(db).to_string(),
            Self::Config(c) => c.get_name_ident(db).text(db).to_string(),
            Self::Resource(r) => r.name(db).ident.text(db).to_string(),
            Self::Task(t) => t.name(db).ident.text(db).to_string(),
            Self::Function(f) => f.get_name_ident(db).text(db).to_string(),
            Self::FunctionBlock(fb) => fb.get_name_ident(db).text(db).to_string(),
            Self::MethodDecl(m) => m.get_name_ident(db).text(db).to_string(),
            Self::Class(c) => c.get_name_ident(db).text(db).to_string(),
            Self::Interface(i) => i.get_name_ident(db).text(db).to_string(),
            Self::DataType(typ) => typ.get_name_ident(db).text(db).to_string(),
            Self::EnumVariant(v) => v.text(db).to_string(),
            Self::StructElement(st) => spec_type_name(db, st.spec(db)),
            Self::Variable((var, _multibits)) => spec_type_name(db, var.spec(db)),
            Self::DirectVariable((dv, _multibits)) => dv.adress(db).text(db).to_string(),
            Self::CallableType(typ) => match typ {
                CallableType::Function(f) => f.get_name_ident(db).text(db).to_string(),
                CallableType::FunctionBlock(fb) => fb.get_name_ident(db).text(db).to_string(),
                CallableType::MethodDecl(m) => m.get_name_ident(db).text(db).to_string(),
            },
            Self::Struct(_) => "STRUCT".into(),
            Self::Enum(_) => "ENUM".into(),
            Self::Array(array) => {
                let elem_type = array.of_type(db).infer(db).type_name(db);
                let dimensions: Vec<String> = array
                    .subranges(db)
                    .iter()
                    .map(|(lower, upper)| {
                        let lower = lower
                            .as_range(db)
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        let upper = upper
                            .as_range(db)
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        format!("{lower}..{upper}")
                    })
                    .collect();
                format!("ARRAY [{}] OF {}", dimensions.join(", "), elem_type)
            }
            Self::ArrayConformand(spec) => {
                let elem_type = spec.infer(db).type_name(db);
                format!("ARRAY [*] OF {}", elem_type)
            }
            Self::SubRange(subrange) => {
                let base_type = subrange._type(db).infer(db).type_name(db);
                let lower = subrange
                    .lower(db)
                    .as_range(db)
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                let upper = subrange
                    .upper(db)
                    .as_range(db)
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                format!("{base_type} ({lower}..{upper})")
            }
            Self::RefTo(spec) => format!("REF_TO {}", spec.infer(db).type_name(db)),
            Self::Null => "NULL".into(),
            Self::Infer(infer) => match infer {
                InferType::Integer(i) => format!("{{integer}} {}", i.ident(db).text(db)),
                InferType::Float(f) => format!("{{float}} {}", f.text(db)),
            },
            Self::Void => "void".into(),
            Self::Never => "{unknown}".into(),
        }
    }

    pub fn path_name(&self, db: &'db dyn WorkspaceDataBase) -> String {
        let scope_id = match self {
            Self::Program(p) => p.get_scope_id(db),
            Self::Function(f) => f.get_scope_id(db),
            Self::FunctionBlock(fb) => fb.get_scope_id(db),
            Self::MethodDecl(m) => m.get_scope_id(db),
            Self::Class(c) => c.get_scope_id(db),
            Self::Interface(i) => i.get_scope_id(db),
            Self::DataType(dt) => dt.get_scope_id(db),
            _ => return Default::default(),
        };

        if scope_id.is_global(db) {
            return Default::default();
        }

        let sema = semantic_index(db, scope_id.file(db));
        let mut result = String::new();
        for scope in sema.scope_iterator(db, scope_id) {
            if let ScopeKind::Namespace(ns) = scope.kind {
                result = format!("{}\n", ns.path(db).to_string(db));
                break;
            }
        }

        result
    }

    /// Returns the fully qualified dotted path for this type (e.g., `Std.Math.Test.test_abs`).
    /// Includes the enclosing namespace path and the type's own name.
    pub fn qualified_path(&self, db: &'db dyn WorkspaceDataBase) -> String {
        let (scope_id, name) = match self {
            Self::Program(p) => (
                p.get_scope_id(db),
                p.get_name_ident(db).text(db).to_string(),
            ),
            Self::Function(f) => (
                f.get_scope_id(db),
                f.get_name_ident(db).text(db).to_string(),
            ),
            Self::FunctionBlock(fb) => (
                fb.get_scope_id(db),
                fb.get_name_ident(db).text(db).to_string(),
            ),
            Self::MethodDecl(m) => (
                m.get_scope_id(db),
                m.get_name_ident(db).text(db).to_string(),
            ),
            Self::Class(c) => (
                c.get_scope_id(db),
                c.get_name_ident(db).text(db).to_string(),
            ),
            Self::Interface(i) => (
                i.get_scope_id(db),
                i.get_name_ident(db).text(db).to_string(),
            ),
            Self::DataType(dt) => (
                dt.get_scope_id(db),
                dt.get_name_ident(db).text(db).to_string(),
            ),
            _ => return Default::default(),
        };

        let sema = semantic_index(db, scope_id.file(db));
        for scope in sema.scope_iterator(db, scope_id) {
            if let ScopeKind::Namespace(ns) = scope.kind {
                return format!("{}.{}", ns.path(db).to_string_dotted(db), name);
            }
        }

        name
    }

    pub fn full_type_name(&self, db: &'db dyn WorkspaceDataBase) -> String {
        match self {
            Self::Struct(ztruct) => {
                let all_elements = ztruct.elements(db);
                if all_elements.is_empty() {
                    return "STRUCT {}".into();
                }
                let fields: Vec<String> = all_elements
                    .iter()
                    .take(10)
                    .map(|elem| {
                        let name = elem.name(db).text(db);
                        let ty = elem.spec(db).infer(db).type_name(db);
                        format!("    {name}: {ty}")
                    })
                    .collect();
                let suffix = if all_elements.len() > 10 {
                    format!("\n    ... ({} more fields)", all_elements.len() - 10)
                } else {
                    String::new()
                };
                format!("STRUCT \n{}{suffix}\n", fields.join(",\n"))
            }
            Self::Enum(enum_) => {
                let all_variants = enum_.variants(db);
                let base = enum_
                    .typ(db)
                    .map(|spec| format!(" ({})", spec.infer(db).type_name(db)))
                    .unwrap_or_default();
                if all_variants.is_empty() {
                    return format!("ENUM{base} {{}}");
                }
                let names: Vec<String> = all_variants
                    .iter()
                    .take(10)
                    .map(|v| v.name.text(db).to_string())
                    .collect();
                let suffix = if all_variants.len() > 10 {
                    format!(", ... ({} more)", all_variants.len() - 10)
                } else {
                    String::new()
                };
                format!("ENUM{base} {}{suffix} ", names.join(", "))
            }
            Self::DataType(typ) => {
                let name = typ.get_name_ident(db).text(db);
                let inner = typ.spec(db).infer(db);
                match &inner {
                    Type::Struct(_) | Type::Enum(_) => {
                        format!("{name}: {}", inner.full_type_name(db))
                    }
                    _ => format!("{name}: {}", inner.type_name(db)),
                }
            }
            _ => self.type_name(db),
        }
    }

    pub fn with_location(&self, db: &'db dyn WorkspaceDataBase, diag: &mut IdeDiagnostic) {
        match self {
            Self::CallableType(typ) => {
                typ.inner_callable().with_location(db, diag);
            }
            Self::Variable((v, multibits)) => match v.spec(db).kind(db) {
                SpecKind::Target(t) => {
                    v.spec(db).infer(db).with_location(db, diag);
                }
                _ => diag.with_related(Related::new(
                    format!(
                        "type is declared by variable '{}' here",
                        v.name(db).text(db)
                    ),
                    v.get_scope_id(db).file(db),
                    v.get_name_span(db),
                )),
            },
            Self::DataType(typ) => {
                diag.with_related(Related::new(
                    format!(
                        "type is defined by '{}' here",
                        typ.get_name_ident(db).text(db)
                    ),
                    typ.scope_id(db).file(db),
                    typ.spec(db).get_span(db),
                ));
            }
            Self::StructElement(elem) => match elem.spec(db).kind(db) {
                SpecKind::Target(t) => {
                    elem.spec(db).infer(db).with_location(db, diag);
                }
                _ => diag.with_related(Related::new(
                    format!(
                        "type is defined by struct field '{}' here",
                        elem.name(db).text(db)
                    ),
                    elem.get_scope_id(db).file(db),
                    elem.get_name_span(db),
                )),
            },
            Self::Function(f) => {
                diag.with_related(Related::new(
                    format!(
                        "FUNCTION '{}' is defined here{}",
                        f.get_name_ident(db).text(db),
                        match f.return_type(db) {
                            Some(ret) =>
                                format!(", with return type '{}'", ret.infer(db).type_name(db)),
                            None => "".to_string(),
                        }
                    ),
                    f.get_scope_id(db).file(db),
                    f.get_name_span(db),
                ));
            }
            Self::MethodDecl(f) => {
                diag.with_related(Related::new(
                    format!(
                        "METHOD '{}' is defined here{}",
                        f.get_name_ident(db).text(db),
                        match f.return_type(db) {
                            Some(ret) =>
                                format!(", with return type '{}'", ret.infer(db).type_name(db)),
                            None => "".to_string(),
                        }
                    ),
                    f.get_scope_id(db).file(db),
                    f.get_name_span(db),
                ));
            }
            Self::FunctionBlock(f) => {
                diag.with_related(Related::new(
                    format!(
                        "FUNCTION_BLOCK '{}' is defined here",
                        f.get_name_ident(db).text(db),
                    ),
                    f.get_scope_id(db).file(db),
                    f.get_name_span(db),
                ));
            }
            Self::Class(f) => {
                diag.with_related(Related::new(
                    format!("CLASS '{}' is defined here", f.get_name_ident(db).text(db),),
                    f.get_scope_id(db).file(db),
                    f.get_name_span(db),
                ));
            }
            Self::Interface(f) => {
                diag.with_related(Related::new(
                    format!(
                        "INTERFACE '{}' is defined here",
                        f.get_name_ident(db).text(db),
                    ),
                    f.get_scope_id(db).file(db),
                    f.get_name_span(db),
                ));
            }
            _ => {}
        }
    }
}

impl<'db> InitExpr<'db> {
    pub fn to_string(&self, db: &'db dyn WorkspaceDataBase) -> &str {
        match self.kind(db) {
            InitExprKind::StructInit { .. } => "STRUCT init",
            InitExprKind::ArrayInit { .. } => "ARRAY init",
            InitExprKind::ArrayIndexedElement { size, .. } => "ARRAY element",
            InitExprKind::StructElement { name, .. } => "STRUCT field",
            InitExprKind::ConstantExpr(expr) => "<expression>",
        }
    }
}

impl<'db> BeginPathExpr<'db> {
    pub fn to_string(&self, db: &'db dyn WorkspaceDataBase) -> &'db str {
        match self.invocation(db) {
            Some(invocation) => match invocation.kind(db) {
                InvocationKind::Super => "SUPER",
                InvocationKind::This => "THIS",
                InvocationKind::SuperBody => "SUPER()",
            },
            None => match self.expr(db) {
                Some(path_expr) => path_expr.ident(db).text(db).as_str(),
                None => "<invalid path>",
            },
        }
    }
}

impl<'db> Expr<'db> {
    pub fn to_string(&self, db: &'db dyn WorkspaceDataBase) -> &str {
        match self.expr(db) {
            ExprKind::PrimaryExpr(primary_expr) => primary_expr.to_string(db),
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => "<arithmetic expression>",
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => "<boolean expression>",
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => "<comparison expression>",
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => "<multiplicative expression>",
            ExprKind::PowerOperator { left, right } => "<power expression>",
            ExprKind::UnaryOperator { expr, operator } => "<unary expression>",
            ExprKind::FoldExpr { .. } => "<fold expression>",
        }
    }
}

impl<'db> PrimaryExpr<'db> {
    pub fn to_string(&self, db: &'db dyn WorkspaceDataBase) -> &str {
        match self {
            PrimaryExpr::Literal(lit) => match lit {
                Elementary::Bool(_) => "BOOL literal",
                Elementary::Byte(_) => "BYTE literal",
                Elementary::Word(_) => "WORD literal",
                Elementary::DWord(_) => "DWORD literal",
                Elementary::LWord(_) => "LWORD literal",
                Elementary::SInt(_) => "SINT literal",
                Elementary::Int(_) => "INT literal",
                Elementary::DInt(_) => "DINT literal",
                Elementary::LInt(_) => "LINT literal",
                Elementary::USInt(_) => "USINT literal",
                Elementary::UInt(_) => "UINT literal",
                Elementary::UDInt(_) => "UDINT literal",
                Elementary::ULInt(_) => "ULINT literal",
                Elementary::Time(_) => "TIME literal",
                Elementary::LTime(_) => "LTIME literal",
                Elementary::Real(_) => "REAL literal",
                Elementary::LReal(_) => "LREAL literal",
                Elementary::DateAndTime(_) => "DATE_AND_TIME literal",
                Elementary::LDateTime(_) => "LDATE_AND_TIME literal",
                Elementary::LDate(_) => "LDATE literal",
                Elementary::Date(_) => "DATE literal",
                Elementary::TimeOfDay(_) => "TIME_OF_DAY literal",
                Elementary::LTod(_) => "LTOD literal",
                Elementary::String(_) => "<string>",
                Elementary::Char(_) => "<char>",
                Elementary::InferInteger(_) => "<integer>",
                Elementary::InferFloat(_) => "<float>",
            },
            PrimaryExpr::VariableAccess(v) => "<variable access>",
            PrimaryExpr::FuncCall(func_call) => func_call.path(db).to_string(db),
            PrimaryExpr::EnumValue { name, variant } => variant.text(db),
            PrimaryExpr::RefValue { value } => match value {
                RefValue::Address(addr) => "<DEREF>",
                RefValue::Null => "NULL",
            },
            PrimaryExpr::ParenthesizedExpr { expr } => expr.to_string(db),
        }
    }
}

impl Integer {
    pub fn to_string(&self, db: &dyn WorkspaceDataBase) -> String {
        match self.kind(db) {
            IntegerKind::Binary => format!("[Binary] {}", self.ident(db).text(db)),
            IntegerKind::Hex => format!("[Hexa] {}", self.ident(db).text(db)),
            IntegerKind::Octal => format!("[Octal] {}", self.ident(db).text(db)),
            IntegerKind::Signed => self.ident(db).text(db).to_string(),
        }
    }
}
