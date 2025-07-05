use std::{ops::Deref, sync::Arc};

use auto_lsp::{
    anyhow::{self},
    default::db::{BaseDatabase, File},
};

use crate::{
    hir::{expression::Expr, variable::{Spec, Subrange}},
    ident::Ident,
    parser::{expression::ParseExpression, ParseInit, ParseSpec, ParseSpecInit, SpecInitResult},
};

// Target

impl<'db> ParseSpecInit<'db> for ast::generated::FqName {
    fn to_spec_init(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        Ok(SpecInitResult::new(self.to_spec(db, file)?, None))
    }
}

impl<'db> ParseSpec<'db> for ast::generated::FqName {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        Ok(Spec::Target(Ident::from_node(db, file, self.target.deref())?))
    }
}

// Simple type

impl<'db> ParseSpec<'db> for ast::generated::SimpleTypeSpec {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        // forwarded to ElemTypeName
        self.children.deref().to_spec(db, file)
    }
}

impl<'db> ParseSpec<'db> for ast::generated::ElemTypeName {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        type AstSpec = ast::generated::ElemTypeName;
        Ok(match self {
            AstSpec::BitStrTypeName(str) => match str.children.deref() {
                ast::generated::BoolName_MultibitsTypeName::BoolName(_) => Spec::Bool,
                ast::generated::BoolName_MultibitsTypeName::MultibitsTypeName(a) => {
                    match a.children.deref() {
                        ast::generated::ByteName_DwordName_LwordName_WordName::ByteName(_) => {
                            Spec::Byte
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::WordName(_) => {
                            Spec::Word
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::DwordName(_) => {
                            Spec::DWord
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::LwordName(_) => {
                            Spec::LWord
                        }
                    }
                }
            },
            AstSpec::NumericTypeName(numeric_type_name) => {
                match numeric_type_name.children.deref() {
                    ast::generated::IntTypeName_RealTypeName::IntTypeName(int) => {
                        int.to_spec(db, file)?
                    }
                    ast::generated::IntTypeName_RealTypeName::RealTypeName(real) => {
                        match real.children.deref() {
                            ast::generated::LrealName_RealName::RealName(_) => Spec::Real,
                            ast::generated::LrealName_RealName::LrealName(_) => Spec::LReal,
                        }
                    }
                }
            }
            AstSpec::DateTypeName(date_type_name) => match date_type_name.children.deref() {
                ast::generated::DateName_LdateName::DateName(_) => Spec::Date,
                ast::generated::DateName_LdateName::LdateName(_) => Spec::LDate,
            },
            AstSpec::TimeTypeName(time_type_name) => match time_type_name.children.deref() {
                ast::generated::LtimeName_TimeName::TimeName(_) => Spec::Time,
                ast::generated::LtimeName_TimeName::LtimeName(_) => Spec::LTime,
            },
            AstSpec::TodTypeName(tod_type_name) => match tod_type_name.children.deref() {
                ast::generated::LtodName_TodName::TodName(_) => Spec::Tod,
                ast::generated::LtodName_TodName::LtodName(_) => Spec::LTod,
            },
            AstSpec::DtTypeName(dt_type_name) => match dt_type_name.children.deref() {
                ast::generated::DtName_LdtName::DtName(_) => Spec::Dt,
                ast::generated::DtName_LdtName::LdtName(_) => Spec::Ldt,
            },
        })
    }
}

impl<'db> ParseSpec<'db> for ast::generated::IntTypeName {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        Ok(match self.children.deref() {
            ast::generated::SignIntTypeName_UnsignIntTypeName::SignIntTypeName(int) => {
                match int.children.deref() {
                    ast::generated::DintName_IntName_LintName_SintName::SintName(_) => Spec::SInt,
                    ast::generated::DintName_IntName_LintName_SintName::IntName(_) => Spec::Int,
                    ast::generated::DintName_IntName_LintName_SintName::DintName(_) => Spec::DInt,
                    ast::generated::DintName_IntName_LintName_SintName::LintName(_) => Spec::LInt,
                }
            }
            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(uint) => {
                match uint.children.deref() {
                    ast::generated::UdintName_UintName_UlintName_UsintName::UsintName(_) => {
                        Spec::USInt
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UintName(_) => {
                        Spec::UInt
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UdintName(_) => {
                        Spec::UDInt
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UlintName(_) => {
                        Spec::ULInt
                    }
                }
            }
        })
    }
}

impl<'db> ParseInit<'db> for ast::generated::SimpleTypeInit {
    fn to_init(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>> {
        Ok(self.children.children.to_expr(db, file)?)
    }
}

// String type

impl<'db> ParseSpec<'db> for ast::generated::StrTypeSpec {
    fn to_spec(&'db self, _: &'db dyn BaseDatabase, _: File) -> anyhow::Result<Spec<'db>> {
        type AstSpec = ast::generated::DByteStrSpec_DChar_SByteStrSpec_SChar;
        Ok(match self.children.deref() {
            AstSpec::DByteStrSpec(_) => Spec::String,
            AstSpec::SByteStrSpec(_) => Spec::WString,
            AstSpec::DChar(_) => Spec::Char,
            AstSpec::SChar(_) => Spec::WChar,
        })
    }
}

// Array type

impl<'db> ParseSpec<'db> for ast::generated::ArrayTypeSpec {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        match self.Type.deref() {
            ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                elem_type_name.to_spec(db, file)
            }
            ast::generated::DataTypeAccess::FqName(target) => {
                target.to_spec(db, file)
            }
        }
    }
}

impl<'db> ParseInit<'db> for ast::generated::ArrayTypeInit {
    fn to_init(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>> {
        todo!()
    }
}

// Array conformand

impl<'db> ParseSpec<'db> for ast::generated::ArrayConformand {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        todo!()
    }
}

impl<'db> ParseInit<'db> for ast::generated::ArrayConformand {
    fn to_init(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>> {
        todo!()
    }
}

// Struct type

impl<'db> ParseSpec<'db> for ast::generated::StructTypeSpec {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        todo!()
    }
}

impl<'db> ParseInit<'db> for ast::generated::StructTypeInit {
    fn to_init(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>> {
        todo!()
    }
}

// RefSpec

impl<'db> ParseSpecInit<'db> for ast::generated::RefSpec {
    fn to_spec_init(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        todo!()
    }
}

impl<'db> ParseSpec<'db> for ast::generated::RefTypeSpec {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        todo!()
    }
}

// Enum type

impl<'db> ParseSpec<'db> for ast::generated::EnumTypeSpec {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        todo!()
    }
}

// Subrange type

impl<'db> ParseSpec<'db> for ast::generated::SubrangeTypeSpec {
    fn to_spec(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>> {
        let spec = self.Type.deref().to_spec(db, file)?;
        let range = self.range.deref();
        let lower = range.lower.children.to_expr(db, file)?;
        let upper = range.upper.children.to_expr(db, file)?;
        Ok(Spec::Subrange(Subrange::new(db, spec, lower, upper)))
    }
}
