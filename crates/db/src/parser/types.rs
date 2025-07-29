use std::ops::Deref;

use auto_lsp::{
    anyhow::{self},
    core::ast::AstNode,
    default::db::{file::File, BaseDatabase},
};

use crate::{
    hir::{
        expressions::expression::Expr,
        interned::namespace::NamespaceAccess,
        pous::variable::{Spec, SpecKind, Subrange},
    },
    parser::{expression::ParseExpression, semantic_index::SemanticIndexBuilder, ParseInit, ParseSpec, ParseSpecInit, SpecInitResult},
};

// Target

impl<'db> ParseSpecInit<'db> for ast::generated::NamespaceAccess {
    fn to_spec_init(
        &self,
        sema: &SemanticIndexBuilder<'db>
    ) -> anyhow::Result<SpecInitResult<'db>> {
        Ok(SpecInitResult::new(self.to_spec(sema)?, None))
    }
}

impl<'db> ParseSpec<'db> for ast::generated::NamespaceAccess {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        Ok(Spec {
            span: self.get_span(),
            kind: SpecKind::Target(NamespaceAccess::from_ast(sema.db, sema.file, self)?),
        })
    }
}

// Simple type

impl<'db> ParseSpec<'db> for ast::generated::SimpleTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        // forwarded to ElemTypeName
        self.children.deref().to_spec(sema)
    }
}

impl<'db> ParseSpec<'db> for ast::generated::DataTypeAccess {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        match self {
            ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                elem_type_name.to_spec(sema)
            }
            ast::generated::DataTypeAccess::NamespaceAccess(target) => target.to_spec(sema),
        }
    }
}

impl<'db> ParseSpec<'db> for ast::generated::ElemTypeName {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        type AstSpec = ast::generated::ElemTypeName;
        Ok(match self {
            AstSpec::BitStrTypeName(str) => match str.children.deref() {
                ast::generated::BoolName_MultibitsTypeName::BoolName(_) => Spec {
                    span: self.get_span(),
                    kind: SpecKind::Bool,
                },
                ast::generated::BoolName_MultibitsTypeName::MultibitsTypeName(a) => {
                    match a.children.deref() {
                        ast::generated::ByteName_DwordName_LwordName_WordName::ByteName(_) => {
                            Spec {
                                span: self.get_span(),
                                kind: SpecKind::Byte,
                            }
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::WordName(_) => {
                            Spec {
                                span: self.get_span(),
                                kind: SpecKind::Word,
                            }
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::DwordName(_) => {
                            Spec {
                                span: self.get_span(),
                                kind: SpecKind::DWord,
                            }
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::LwordName(_) => {
                            Spec {
                                span: self.get_span(),
                                kind: SpecKind::LWord,
                            }
                        }
                    }
                }
            },
            AstSpec::NumericTypeName(numeric_type_name) => {
                match numeric_type_name.children.deref() {
                    ast::generated::IntTypeName_RealTypeName::IntTypeName(int) => {
                        int.to_spec(sema)?
                    }
                    ast::generated::IntTypeName_RealTypeName::RealTypeName(real) => {
                        match real.children.deref() {
                            ast::generated::LrealName_RealName::RealName(_) => Spec {
                                span: self.get_span(),
                                kind: SpecKind::Real,
                            },
                            ast::generated::LrealName_RealName::LrealName(_) => Spec {
                                span: self.get_span(),
                                kind: SpecKind::LReal,
                            },
                        }
                    }
                }
            }
            AstSpec::AnyDateTypeName(date_type_name) => match date_type_name {
                ast::generated::AnyDateTypeName::DateTypeName(_) => Spec {
                    span: self.get_span(),
                    kind: SpecKind::Date,
                },
                ast::generated::AnyDateTypeName::LDateTypeName(_) => Spec {
                    span: self.get_span(),
                    kind: SpecKind::LDate,
                },
            },
            AstSpec::AnyTimeTypeName(time_type_name) => match time_type_name {
                ast::generated::AnyTimeTypeName::TimeTypeName(_) => Spec {
                    span: self.get_span(),
                    kind: SpecKind::Time,
                },
                ast::generated::AnyTimeTypeName::LTimeTypeName(_) => Spec {
                    span: self.get_span(),
                    kind: SpecKind::LTime,
                },
            },
            AstSpec::AnyTodTypeName(tod_type_name) => match tod_type_name {
                ast::generated::AnyTodTypeName::TodTypeName(_) => Spec {
                    span: self.get_span(),
                    kind: SpecKind::Tod,
                },
                ast::generated::AnyTodTypeName::LtodTypeName(_) => Spec {
                    span: self.get_span(),
                    kind: SpecKind::LTod,
                },
            },
            AstSpec::AnyDtTypeName(dt_type_name) => match dt_type_name {
                ast::generated::AnyDtTypeName::DtTypeName(_) => Spec {
                    span: self.get_span(),
                    kind: SpecKind::Dt,
                },
                ast::generated::AnyDtTypeName::LDtTypeName(_) => Spec {
                    span: self.get_span(),
                    kind: SpecKind::Ldt,
                },
            },
        })
    }
}

impl<'db> ParseSpec<'db> for ast::generated::IntTypeName {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        Ok(match self.children.deref() {
            ast::generated::SignIntTypeName_UnsignIntTypeName::SignIntTypeName(int) => {
                match int.children.deref() {
                    ast::generated::DintName_IntName_LintName_SintName::SintName(_) => Spec {
                        span: self.get_span(),
                        kind: SpecKind::SInt,
                    },
                    ast::generated::DintName_IntName_LintName_SintName::IntName(_) => Spec {
                        span: self.get_span(),
                        kind: SpecKind::Int,
                    },
                    ast::generated::DintName_IntName_LintName_SintName::DintName(_) => Spec {
                        span: self.get_span(),
                        kind: SpecKind::DInt,
                    },
                    ast::generated::DintName_IntName_LintName_SintName::LintName(_) => Spec {
                        span: self.get_span(),
                        kind: SpecKind::LInt,
                    },
                }
            }
            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(uint) => {
                match uint.children.deref() {
                    ast::generated::UdintName_UintName_UlintName_UsintName::UsintName(_) => Spec {
                        span: self.get_span(),
                        kind: SpecKind::USInt,
                    },
                    ast::generated::UdintName_UintName_UlintName_UsintName::UintName(_) => Spec {
                        span: self.get_span(),
                        kind: SpecKind::UInt,
                    },
                    ast::generated::UdintName_UintName_UlintName_UsintName::UdintName(_) => Spec {
                        span: self.get_span(),
                        kind: SpecKind::UDInt,
                    },
                    ast::generated::UdintName_UintName_UlintName_UsintName::UlintName(_) => Spec {
                        span: self.get_span(),
                        kind: SpecKind::ULInt,
                    },
                }
            }
        })
    }
}

impl<'db> ParseInit<'db> for ast::generated::SimpleTypeInit {
    fn to_init(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Expr<'db>> {
        Ok(self.children.children.to_expr(sema)?)
    }
}

// String type

impl<'db> ParseSpec<'db> for ast::generated::StrTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        type AstSpec = ast::generated::DByteStrSpec_DChar_SByteStrSpec_SChar;
        Ok(match self.children.deref() {
            AstSpec::DByteStrSpec(_) => Spec {
                span: self.get_span(),
                kind: SpecKind::String,
            },

            AstSpec::SByteStrSpec(_) => Spec {
                span: self.get_span(),
                kind: SpecKind::WString,
            },

            AstSpec::DChar(_) => Spec {
                span: self.get_span(),
                kind: SpecKind::Char,
            },

            AstSpec::SChar(_) => Spec {
                span: self.get_span(),
                kind: SpecKind::WChar,
            },
        })
    }
}

// Array type

impl<'db> ParseSpec<'db> for ast::generated::ArrayTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        match self.Type.deref() {
            ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                elem_type_name.to_spec(sema)
            }
            ast::generated::DataTypeAccess::NamespaceAccess(target) => target.to_spec(sema),
        }
    }
}

impl<'db> ParseInit<'db> for ast::generated::ArrayTypeInit {
    fn to_init(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Expr<'db>> {
        todo!()
    }
}

// Array conformand

impl<'db> ParseSpec<'db> for ast::generated::ArrayConformand {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        todo!()
    }
}

impl<'db> ParseInit<'db> for ast::generated::ArrayConformand {
    fn to_init(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Expr<'db>> {
        todo!()
    }
}

// Struct type

impl<'db> ParseSpec<'db> for ast::generated::StructTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        todo!()
    }
}

impl<'db> ParseInit<'db> for ast::generated::StructTypeInit {
    fn to_init(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Expr<'db>> {
        todo!()
    }
}

// RefSpec

impl<'db> ParseSpecInit<'db> for ast::generated::RefSpec {
    fn to_spec_init(
        &self,
        sema: &SemanticIndexBuilder<'db>
    ) -> anyhow::Result<SpecInitResult<'db>> {
        todo!()
    }
}

impl<'db> ParseSpec<'db> for ast::generated::RefTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        todo!()
    }
}

// Enum type

impl<'db> ParseSpec<'db> for ast::generated::EnumTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        todo!()
    }
}

// Subrange type

impl<'db> ParseSpec<'db> for ast::generated::SubrangeTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        let spec = self.Type.deref().to_spec(sema)?;
        let range = self.range.deref();
        let lower = range.lower.children.to_expr(sema)?;
        let upper = range.upper.children.to_expr(sema)?;
        Ok(Spec {
            span: self.get_span(),
            kind: SpecKind::Subrange(Subrange::new(sema.db, spec, lower, upper)),
        })
    }
}
