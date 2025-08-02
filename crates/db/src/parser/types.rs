use std::ops::Deref;

use auto_lsp::{
    anyhow::{self},
    core::ast::AstNode,
};

use crate::{
    hir::{
        expressions::{
            expression::Expr,
            spec::{CompositeSpecKind, SimpleSpecKind, Spec, SpecKind, SubRange},
        },
        interned::namespace::NamespaceAccess,
    },
    parser::{
        expression::ParseExpression, semantic_index::SemanticIndexBuilder, ParseInit, ParseSpec,
        ParseSpecInit, SpecInitResult,
    },
};

// Target

impl<'db> ParseSpecInit<'db> for ast::generated::NamespaceAccess {
    fn to_spec_init(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        Ok(SpecInitResult::new(self.to_spec(sema)?, None))
    }
}

impl<'db> ParseSpec<'db> for ast::generated::NamespaceAccess {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        Ok(Spec::new(
            sema.db,
            self.get_span(),
            SpecKind::Target(NamespaceAccess::from_ast(sema.db, sema.file, self)?),
            sema.current_scope,
        ))
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
                ast::generated::BoolName_MultibitsTypeName::BoolName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::Bool),
                    sema.current_scope,
                ),
                ast::generated::BoolName_MultibitsTypeName::MultibitsTypeName(a) => {
                    match a.children.deref() {
                        ast::generated::ByteName_DwordName_LwordName_WordName::ByteName(_) => {
                            Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::Simple(SimpleSpecKind::Byte),
                                sema.current_scope,
                            )
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::WordName(_) => {
                            Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::Simple(SimpleSpecKind::Word),
                                sema.current_scope,
                            )
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::DwordName(_) => {
                            Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::Simple(SimpleSpecKind::DWord),
                                sema.current_scope,
                            )
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::LwordName(_) => {
                            Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::Simple(SimpleSpecKind::LWord),
                                sema.current_scope,
                            )
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
                            ast::generated::LrealName_RealName::RealName(_) => Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::Simple(SimpleSpecKind::Real),
                                sema.current_scope,
                            ),
                            ast::generated::LrealName_RealName::LrealName(_) => Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::Simple(SimpleSpecKind::LReal),
                                sema.current_scope,
                            ),
                        }
                    }
                }
            }
            AstSpec::AnyDateTypeName(date_type_name) => match date_type_name {
                ast::generated::AnyDateTypeName::DateTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::Date),
                    sema.current_scope,
                ),
                ast::generated::AnyDateTypeName::LDateTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::LDate),
                    sema.current_scope,
                ),
            },
            AstSpec::AnyTimeTypeName(time_type_name) => match time_type_name {
                ast::generated::AnyTimeTypeName::TimeTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::Time),
                    sema.current_scope,
                ),
                ast::generated::AnyTimeTypeName::LTimeTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::LTime),
                    sema.current_scope,
                ),
            },
            AstSpec::AnyTodTypeName(tod_type_name) => match tod_type_name {
                ast::generated::AnyTodTypeName::TodTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::Tod),
                    sema.current_scope,
                ),
                ast::generated::AnyTodTypeName::LtodTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::LTod),
                    sema.current_scope,
                ),
            },
            AstSpec::AnyDtTypeName(dt_type_name) => match dt_type_name {
                ast::generated::AnyDtTypeName::DtTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::Dt),
                    sema.current_scope,
                ),
                ast::generated::AnyDtTypeName::LDtTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::Ldt),
                    sema.current_scope,
                ),
            },
        })
    }
}

impl<'db> ParseSpec<'db> for ast::generated::IntTypeName {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        Ok(match self.children.deref() {
            ast::generated::SignIntTypeName_UnsignIntTypeName::SignIntTypeName(int) => {
                match int.children.deref() {
                    ast::generated::DintName_IntName_LintName_SintName::SintName(_) => Spec::new(
                        sema.db,
                        self.get_span(),
                        SpecKind::Simple(SimpleSpecKind::SInt),
                        sema.current_scope,
                    ),
                    ast::generated::DintName_IntName_LintName_SintName::IntName(_) => Spec::new(
                        sema.db,
                        self.get_span(),
                        SpecKind::Simple(SimpleSpecKind::Int),
                        sema.current_scope,
                    ),
                    ast::generated::DintName_IntName_LintName_SintName::DintName(_) => Spec::new(
                        sema.db,
                        self.get_span(),
                        SpecKind::Simple(SimpleSpecKind::DInt),
                        sema.current_scope,
                    ),
                    ast::generated::DintName_IntName_LintName_SintName::LintName(_) => Spec::new(
                        sema.db,
                        self.get_span(),
                        SpecKind::Simple(SimpleSpecKind::LInt),
                        sema.current_scope,
                    ),
                }
            }
            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(uint) => {
                match uint.children.deref() {
                    ast::generated::UdintName_UintName_UlintName_UsintName::UsintName(_) => {
                        Spec::new(
                            sema.db,
                            self.get_span(),
                            SpecKind::Simple(SimpleSpecKind::USInt),
                            sema.current_scope,
                        )
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UintName(_) => {
                        Spec::new(
                            sema.db,
                            self.get_span(),
                            SpecKind::Simple(SimpleSpecKind::UInt),
                            sema.current_scope,
                        )
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UdintName(_) => {
                        Spec::new(
                            sema.db,
                            self.get_span(),
                            SpecKind::Simple(SimpleSpecKind::UDInt),
                            sema.current_scope,
                        )
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UlintName(_) => {
                        Spec::new(
                            sema.db,
                            self.get_span(),
                            SpecKind::Simple(SimpleSpecKind::ULInt),
                            sema.current_scope,
                        )
                    }
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
            AstSpec::DByteStrSpec(_) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::Simple(SimpleSpecKind::WString),
                sema.current_scope,
            ),

            AstSpec::SByteStrSpec(_) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::Simple(SimpleSpecKind::String),
                sema.current_scope,
            ),

            AstSpec::DChar(_) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::Simple(SimpleSpecKind::WChar),
                sema.current_scope,
            ),

            AstSpec::SChar(_) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::Simple(SimpleSpecKind::Char),
                sema.current_scope,
            ),
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
        sema: &SemanticIndexBuilder<'db>,
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
        Ok(Spec::new(
            sema.db,
            self.get_span(),
            SpecKind::Composite(CompositeSpecKind::Subrange(SubRange {
                _type: Box::new(spec),
                lower,
                upper,
            })),
            sema.current_scope,
        ))
    }
}
