use auto_lsp::default::db::BaseDatabase;
use compact_str::CompactString;
use ide_diagnostic::{IdeDiagnostic, Related};

use crate::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                Elementary, Expr, ExprKind, InitExpr, InitExprKind, PrimaryExpr, RefValue,
            },
            spec::{ElementarySpec, Spec, SpecKind},
        },
        pous::function::Function,
    },
    hir_ty::ty::{CallableType, InferType, Type},
};

impl<'db> Type<'db> {
    #[cfg(debug_assertions)]
    pub fn kind(&self) -> &'static str {
        match self {
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

    pub fn with_name(&self, db: &'db dyn BaseDatabase) -> Option<String> {
        Some(
            match self {
                Self::Function(f) => f.get_name_ident(db).text(db),
                Self::FunctionBlock(fb) => fb.get_name_ident(db).text(db),
                Self::MethodDecl(m) => m.get_name_ident(db).text(db),
                Self::Class(c) => c.get_name_ident(db).text(db),
                Self::Interface(i) => i.get_name_ident(db).text(db),
                Self::DataType(typ) => typ.get_name_ident(db).text(db),
                _ => None?,
            }
            .to_string(),
        )
    }
    pub fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            Self::Elementary(elem) => match elem {
                ElementarySpec::Bool => "BOOL",
                ElementarySpec::REDGEBool => "BOOL (RISING EDGE)",
                ElementarySpec::FEDGEBool => "BOOl (FALLING EDGE)",
                ElementarySpec::Byte => "BYTE",
                ElementarySpec::Word => "WORD",
                ElementarySpec::DWord => "DWORD",
                ElementarySpec::LWord => "LWORD",
                ElementarySpec::SInt => "SINT",
                ElementarySpec::USInt => "USINT",
                ElementarySpec::UInt => "UINT",
                ElementarySpec::Int => "INT",
                ElementarySpec::DInt => "DINT",
                ElementarySpec::UDInt => "UDINT",
                ElementarySpec::LInt => "LINT",
                ElementarySpec::ULInt => "ULINT",
                ElementarySpec::Real => "REAL",
                ElementarySpec::LReal => "LREAL",
                ElementarySpec::String => "STRING",
                ElementarySpec::WString => "WSTRING",
                ElementarySpec::Char => "CHAR",
                ElementarySpec::WChar => "WCHAR",
                ElementarySpec::Date => "DATE",
                ElementarySpec::LDate => "LDATE",
                ElementarySpec::DateAndTime => "DT",
                ElementarySpec::LDateTime => "LDT",
                ElementarySpec::Time => "TIME",
                ElementarySpec::LTime => "LTIME",
                ElementarySpec::Tod => "TOD",
                ElementarySpec::LTod => "LTOD",
            }
            .into(),
            Self::Function(f) => format!("FUNCTION: {}", f.get_name_ident(db).text(db)),
            Self::FunctionBlock(fb) => {
                format!("FUNCTION_BLOCK: {}", fb.get_name_ident(db).text(db))
            }
            Self::MethodDecl(m) => format!("METHOD: {}", m.get_name_ident(db).text(db)),
            Self::Class(c) => format!("CLASS: {}", c.get_name_ident(db).text(db)),
            Self::Interface(i) => format!("INTERFACE: {}", i.get_name_ident(db).text(db)),
            Self::DataType(typ) => match typ.spec(db).kind(db) {
                SpecKind::Target(e) => Type::new_spec(db, typ.spec(db)).type_name(db),
                _ => Type::new_spec(db, typ.spec(db)).type_name(db),
            },
            Self::Enum(_) => "ENUM".into(),
            Self::Struct(_) => "STRUCT".into(),
            Self::CallableType(typ) => match typ {
                CallableType::Function(f) => format!("FUNCTION: {}", f.get_name_ident(db).text(db)),
                CallableType::FunctionBlock(fb) => {
                    format!("FUNCTION_BLOCK: {}", fb.get_name_ident(db).text(db))
                }
                CallableType::MethodDecl(m) => format!("METHOD: {}", m.get_name_ident(db).text(db)),
            },
            Self::Never => "{unknown}".into(),
            Self::Void => "void".into(),
            Self::StructElement(st) => Type::new_spec(db, st.spec(db)).type_name(db),
            Self::Variable(var) => Type::new_spec(db, var.spec(db)).type_name(db),
            Self::Infer(infer) => match infer {
                InferType::Integer(i) => format!("{{integer}} {}", i.ident(db).text(db)),
                InferType::Float(f) => format!("{{float}} {}", f.text(db)),
            },
            _ => self.full_type_name(db),
        }
    }

    pub fn full_type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            Self::Array(array) => {
                let elem_type = Type::new_spec(db, array.of_type(db)).type_name(db);
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
                        format!("[{lower}..{upper}]")
                    })
                    .collect();
                format!("ARRAY {} OF {}", dimensions.join(" "), elem_type)
            }
            Self::Enum(enum_) => format!("ENUM ({} members)", enum_.variants(db).len()),
            Self::SubRange(subrange) => {
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

                format!("SUBRANGE ({lower}..{upper})")
            }
            Self::Struct(ztruct) => format!("STRUCT ({} members)", ztruct.elements(db).len()),
            Self::RefTo(ref_to) => format!("REF TO {}", Type::new_spec(db, *ref_to).type_name(db),),
            _ => self.type_name(db),
        }
    }

    pub fn with_location(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        match self {
            Self::Variable(v) => match v.spec(db).kind(db) {
                SpecKind::Target(t) => {
                    Type::new_spec(db, v.spec(db)).with_location(db, diag);
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
                    Type::new_spec(db, elem.spec(db)).with_location(db, diag);
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
                            Some(ret) => format!(
                                ", with return type '{}'",
                                Type::new_spec(db, *ret).type_name(db)
                            ),
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
                            Some(ret) => format!(
                                ", with return type '{}'",
                                Type::new_spec(db, *ret).type_name(db)
                            ),
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
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> &str {
        match self.kind(db) {
            InitExprKind::StructInit { .. } => "STRUCT init",
            InitExprKind::ArrayInit { .. } => "ARRAY init",
            InitExprKind::ArrayIndexedElement { size, .. } => "ARRAY element",
            InitExprKind::StructElement { name, .. } => "STRUCT field",
            InitExprKind::ConstantExpr(expr) => "<expression>",
        }
    }
}

impl<'db> Expr<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> &str {
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
        }
    }
}

impl<'db> PrimaryExpr<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> &str {
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
                Elementary::AnyString(_) => "STRING literal",
                Elementary::AnyChar(_) => "CHAR literal",
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
