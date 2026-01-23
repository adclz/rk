use auto_lsp::anyhow::{self};
use auto_lsp::core::ast::AstNode;

use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::expressions::spec::Array;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use crate::{
    builder::{
        ParseSpec, ParseSpecInit, SpecInitResult,
        expression::{ParseExpr, ParseExpression, ParseVariableAccess},
        semantic_index::SemanticIndexBuilder,
    },
    check::errors::analysis_error::AnalysisError,
    hir_def::{
        expressions::{
            expression::{InitExpr, InitExprKind, Integer, IntegerKind, MultibitsPart},
            spec::{
                ElementarySpec, Enum, EnumVariant, Spec, SpecKind, Struct, StructElement, SubRange,
            },
        },
        interned::identifier::Ident,
    },
};

// Target

impl<'db> ParseSpecInit<'db> for ast::generated::NamespaceAccess {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, AnalysisError<'db>> {
        Ok(SpecInitResult::new(self.to_spec(sema)?, None))
    }
}

impl<'db> ParseSpec<'db> for ast::generated::NamespaceAccess {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        Ok(Spec::new(
            sema.db,
            SpecKind::Target(SpanNamespaceAccess::from_ast(sema.db, sema, self)?),
            self.into(),
            sema.current_scope,
        ))
    }
}

// Simple type

impl<'db> ParseSpec<'db> for ast::generated::SimpleTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        self.children.cast(sema.ast).to_spec(sema)
    }
}

impl<'db> ParseSpec<'db> for ast::generated::DataTypeAccess {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        match self {
            ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                elem_type_name.to_spec(sema)
            }
            ast::generated::DataTypeAccess::NamespaceAccess(target) => target.to_spec(sema),
        }
    }
}

impl<'db> ParseSpec<'db> for ast::generated::ElemTypeName {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        type AstSpec = ast::generated::ElemTypeName;
        Ok(match self {
            AstSpec::BitStrTypeName(str) => match str.children.cast(sema.ast) {
                ast::generated::BoolName_MultibitsTypeName::BoolName(_) => Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::Bool),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::BoolName_MultibitsTypeName::MultibitsTypeName(a) => {
                    match a.children.cast(sema.ast) {
                        ast::generated::ByteName_DwordName_LwordName_WordName::ByteName(_) => {
                            Spec::new(
                                sema.db,
                                SpecKind::Simple(ElementarySpec::Byte),
                                self.into(),
                                sema.current_scope,
                            )
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::WordName(_) => {
                            Spec::new(
                                sema.db,
                                SpecKind::Simple(ElementarySpec::Word),
                                self.into(),
                                sema.current_scope,
                            )
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::DwordName(_) => {
                            Spec::new(
                                sema.db,
                                SpecKind::Simple(ElementarySpec::DWord),
                                self.into(),
                                sema.current_scope,
                            )
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::LwordName(_) => {
                            Spec::new(
                                sema.db,
                                SpecKind::Simple(ElementarySpec::LWord),
                                self.into(),
                                sema.current_scope,
                            )
                        }
                    }
                }
            },
            AstSpec::NumericTypeName(numeric_type_name) => {
                match numeric_type_name.children.cast(sema.ast) {
                    ast::generated::IntTypeName_RealTypeName::IntTypeName(int) => {
                        int.to_spec(sema)?
                    }
                    ast::generated::IntTypeName_RealTypeName::RealTypeName(real) => {
                        match real.children.cast(sema.ast) {
                            ast::generated::LrealName_RealName::RealName(_) => Spec::new(
                                sema.db,
                                SpecKind::Simple(ElementarySpec::Real),
                                self.into(),
                                sema.current_scope,
                            ),
                            ast::generated::LrealName_RealName::LrealName(_) => Spec::new(
                                sema.db,
                                SpecKind::Simple(ElementarySpec::LReal),
                                self.into(),
                                sema.current_scope,
                            ),
                        }
                    }
                }
            }
            AstSpec::AnyDateTypeName(date_type_name) => match date_type_name {
                ast::generated::AnyDateTypeName::DateTypeName(_) => Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::Date),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::AnyDateTypeName::LDateTypeName(_) => Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::LDate),
                    self.into(),
                    sema.current_scope,
                ),
            },
            AstSpec::AnyTimeTypeName(time_type_name) => match time_type_name {
                ast::generated::AnyTimeTypeName::TimeTypeName(_) => Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::Time),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::AnyTimeTypeName::LTimeTypeName(_) => Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::LTime),
                    self.into(),
                    sema.current_scope,
                ),
            },
            AstSpec::AnyTodTypeName(tod_type_name) => match tod_type_name {
                ast::generated::AnyTodTypeName::TodTypeName(_) => Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::Tod),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::AnyTodTypeName::LtodTypeName(_) => Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::LTod),
                    self.into(),
                    sema.current_scope,
                ),
            },
            AstSpec::AnyDtTypeName(dt_type_name) => match dt_type_name {
                ast::generated::AnyDtTypeName::DtTypeName(_) => Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::DateAndTime),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::AnyDtTypeName::LDtTypeName(_) => Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::LDateTime),
                    self.into(),
                    sema.current_scope,
                ),
            },
        })
    }
}

impl<'db> ParseSpec<'db> for ast::generated::IntTypeName {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        Ok(match self.children.cast(sema.ast) {
            ast::generated::SignIntTypeName_UnsignIntTypeName::SignIntTypeName(int) => {
                match int.children.cast(sema.ast) {
                    ast::generated::DintName_IntName_LintName_SintName::SintName(_) => Spec::new(
                        sema.db,
                        SpecKind::Simple(ElementarySpec::SInt),
                        self.into(),
                        sema.current_scope,
                    ),
                    ast::generated::DintName_IntName_LintName_SintName::IntName(_) => Spec::new(
                        sema.db,
                        SpecKind::Simple(ElementarySpec::Int),
                        self.into(),
                        sema.current_scope,
                    ),
                    ast::generated::DintName_IntName_LintName_SintName::DintName(_) => Spec::new(
                        sema.db,
                        SpecKind::Simple(ElementarySpec::DInt),
                        self.into(),
                        sema.current_scope,
                    ),
                    ast::generated::DintName_IntName_LintName_SintName::LintName(_) => Spec::new(
                        sema.db,
                        SpecKind::Simple(ElementarySpec::LInt),
                        self.into(),
                        sema.current_scope,
                    ),
                }
            }
            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(uint) => {
                match uint.children.cast(sema.ast) {
                    ast::generated::UdintName_UintName_UlintName_UsintName::UsintName(_) => {
                        Spec::new(
                            sema.db,
                            SpecKind::Simple(ElementarySpec::USInt),
                            self.into(),
                            sema.current_scope,
                        )
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UintName(_) => {
                        Spec::new(
                            sema.db,
                            SpecKind::Simple(ElementarySpec::UInt),
                            self.into(),
                            sema.current_scope,
                        )
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UdintName(_) => {
                        Spec::new(
                            sema.db,
                            SpecKind::Simple(ElementarySpec::UDInt),
                            self.into(),
                            sema.current_scope,
                        )
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UlintName(_) => {
                        Spec::new(
                            sema.db,
                            SpecKind::Simple(ElementarySpec::ULInt),
                            self.into(),
                            sema.current_scope,
                        )
                    }
                }
            }
        })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::SimpleTypeInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<InitExpr<'db>, AnalysisError<'db>> {
        Ok(InitExpr::new(
            sema.db,
            InitExprKind::ConstantExpr(
                self.children
                    .cast(sema.ast)
                    .children
                    .cast(sema.ast)
                    .to_expr(sema)?,
            ),
            self.into(),
            sema.current_scope,
        ))
    }
}

// String type

impl<'db> ParseSpec<'db> for ast::generated::StrTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        type AstSpec = ast::generated::DByteStrSpec_DChar_SByteStrSpec_SChar;
        Ok(match self.children.cast(sema.ast) {
            AstSpec::DByteStrSpec(_) => Spec::new(
                sema.db,
                SpecKind::Simple(ElementarySpec::WString),
                self.into(),
                sema.current_scope,
            ),

            AstSpec::SByteStrSpec(_) => Spec::new(
                sema.db,
                SpecKind::Simple(ElementarySpec::String),
                self.into(),
                sema.current_scope,
            ),

            AstSpec::DChar(_) => Spec::new(
                sema.db,
                SpecKind::Simple(ElementarySpec::WChar),
                self.into(),
                sema.current_scope,
            ),

            AstSpec::SChar(_) => Spec::new(
                sema.db,
                SpecKind::Simple(ElementarySpec::Char),
                self.into(),
                sema.current_scope,
            ),
        })
    }
}

// Array type

impl<'db> ParseSpec<'db> for ast::generated::ArrayTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        let mut ranges = vec![];

        for range in &self.ranges.cast(sema.ast).children {
            let lower = range
                .cast(sema.ast)
                .lower
                .cast(sema.ast)
                .children
                .cast(sema.ast)
                .to_expr(sema)?;
            let upper = range
                .cast(sema.ast)
                .upper
                .cast(sema.ast)
                .children
                .cast(sema.ast)
                .to_expr(sema)?;
            ranges.push((lower, upper))
        }

        let kind = match self.Type.cast(sema.ast) {
            ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                elem_type_name.to_spec(sema)
            }
            ast::generated::DataTypeAccess::NamespaceAccess(target) => target.to_spec(sema),
        }?;

        Ok(Spec::new(
            sema.db,
            SpecKind::Array(Array::new(sema.db, ranges, kind)),
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> ParseExpr<'db> for ast::generated::ArrayTypeInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        let values = self
            .children
            .iter()
            .map(|elem| elem.cast(sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<InitExpr<'db>>, AnalysisError<'db>>>()?;

        Ok(InitExpr::new(
            sema.db,
            InitExprKind::ArrayInit { values },
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> ParseExpr<'db> for ast::generated::StructTypeInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        let values = self
            .children
            .iter()
            .map(|elem| elem.cast(sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<InitExpr<'db>>, AnalysisError<'db>>>()?;

        Ok(InitExpr::new(
            sema.db,
            InitExprKind::StructInit { values },
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> ParseExpr<'db> for ast::generated::InitElem {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        type InitElem = ast::generated::ERRFuncCallInInit_ArrayIndexElem_ArrayInit_ConstantExpr_StructElem_StructInit;
        match self.children.cast(sema.ast) {
            InitElem::ArrayIndexElem(array_type_init) => array_type_init.parse(sema),
            InitElem::ArrayInit(expr) => expr.parse(sema),
            InitElem::StructElem(struct_elem_init) => struct_elem_init.parse(sema),
            InitElem::StructInit(struct_type_init) => struct_type_init.parse(sema),
            InitElem::ConstantExpr(expr) => Ok(InitExpr::new(
                sema.db,
                InitExprKind::ConstantExpr(expr.children.cast(sema.ast).to_expr(sema)?),
                self.into(),
                sema.current_scope,
            )),
            InitElem::ERRFuncCallInInit(err) => Err(AnalysisError::Syntax(
                SyntaxError::FunctionCallInInitExpression(err.get_span()),
            )),
        }
    }
}

impl<'db> ParseExpr<'db> for ast::generated::ArrayInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        let values = self
            .values
            .cast(sema.ast)
            .children
            .iter()
            .map(|elem| elem.cast(sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<InitExpr<'db>>, AnalysisError<'db>>>()?;

        Ok(InitExpr::new(
            sema.db,
            InitExprKind::ArrayInit { values },
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> ParseExpr<'db> for ast::generated::ArrayIndexElem {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        let index = SpanIdent::from_node(sema.db, sema, self.index.cast(sema.ast))?;

        let values = self
            .values
            .cast(sema.ast)
            .children
            .iter()
            .map(|elem| elem.cast(sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<InitExpr<'db>>, AnalysisError<'db>>>()?;

        Ok(InitExpr::new(
            sema.db,
            InitExprKind::ArrayIndexedElement {
                size: index,
                values,
            },
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> ParseExpr<'db> for ast::generated::StructInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        let values = self
            .children
            .iter()
            .map(|elem| elem.cast(sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<InitExpr<'db>>, AnalysisError<'db>>>()?;

        Ok(InitExpr::new(
            sema.db,
            InitExprKind::StructInit { values },
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> ParseExpr<'db> for ast::generated::StructElem {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, AnalysisError<'db>> {
        let name = SpanIdent::from_node(sema.db, sema, self.name.cast(sema.ast))?;
        let value = Box::new(self.value.cast(sema.ast).parse(sema)?);

        Ok(InitExpr::new(
            sema.db,
            InitExprKind::StructElement { name, value },
            self.into(),
            sema.current_scope,
        ))
    }
}

// Array conformand

impl<'db> ParseSpec<'db> for ast::generated::ArrayConformand {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        Ok(Spec::new(
            sema.db,
            SpecKind::ArrayConformand(self.children.cast(sema.ast).to_spec(sema)?),
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> ParseSpec<'db> for ast::generated::StructTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        let mut elements = vec![];

        for elem in &self.children {
            type Spec = ast::generated::ArrayTypeSpec_EnumTypeSpec_SimpleTypeSpec_StructTypeSpec_SubrangeTypeSpec;

            let spec = match elem.cast(sema.ast).spec.cast(sema.ast) {
                Spec::ArrayTypeSpec(elem) => elem.to_spec(sema)?,
                Spec::SimpleTypeSpec(init) => init.to_spec(sema)?,
                Spec::EnumTypeSpec(en) => en.to_spec(sema)?,
                Spec::SubrangeTypeSpec(sub) => sub.to_spec(sema)?,
                Spec::StructTypeSpec(st) => st.to_spec(sema)?,
            };

            type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

            let init = if let Some(init) =
                elem.cast(sema.ast).init.as_ref().map(|i| i.cast(sema.ast))
            {
                match init {
                    Init::ArrayTypeInit(array_type_init) => Some(array_type_init.parse(sema)?),
                    Init::SimpleTypeInit(simple_type_init) => Some(simple_type_init.parse(sema)?),
                    Init::StructTypeInit(struct_type_init) => Some(struct_type_init.parse(sema)?),
                }
            } else {
                None
            };

            let name =
                Ident::from_node(sema.db, sema.file, elem.cast(sema.ast).name.cast(sema.ast))?;

            let (located, multibits) = if let Some(attrs) = &elem.cast(sema.ast).attributes {
                let located = attrs
                    .cast(sema.ast)
                    .located
                    .cast(sema.ast)
                    .children
                    .cast(sema.ast)
                    .to_access(sema)
                    .ok();
                let multibits = attrs
                    .cast(sema.ast)
                    .multibits
                    .as_ref()
                    .map(|mb| mb.cast(sema.ast).to_multibits(sema))
                    .transpose()?;
                (located, multibits)
            } else {
                (None, None)
            };

            elements.push(StructElement::new(
                sema.db,
                name,
                elem.cast(sema.ast).name.cast(sema.ast).into(),
                located,
                multibits,
                spec,
                init,
                elem.cast(sema.ast).into(),
                sema.current_scope,
            ));
        }

        Ok(Spec::new(
            sema.db,
            SpecKind::Struct(Struct::new(sema.db, self.overlap.is_some(), elements)),
            self.into(),
            sema.current_scope,
        ))
    }
}

pub trait ParseMultiBits<'db> {
    fn to_multibits(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<MultibitsPart, AnalysisError<'db>>;
}

impl<'db> ParseMultiBits<'db> for ast::generated::MultibitPartAccess {
    fn to_multibits(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<MultibitsPart, AnalysisError<'db>> {
        let offset = Integer::new(
            sema.db,
            Ident::from_node(
                sema.db,
                sema.file,
                self.path.cast(sema.ast).children.cast(sema.ast),
            )?,
            IntegerKind::Signed,
        );

        match &self.path.cast(sema.ast).access {
            Some(access) => Ok(MultibitsPart::AccessOffset {
                offset,
                access: Ident::from_node(sema.db, sema.file, access.cast(sema.ast))?,
            }),
            None => Ok(MultibitsPart::Offset(offset)),
        }
    }
}

// RefSpec

impl<'db> ParseSpecInit<'db> for ast::generated::RefSpec {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, AnalysisError<'db>> {
        Ok(SpecInitResult {
            spec: self.children.cast(sema.ast).to_spec(sema)?,
            init: None,
        })
    }
}

impl<'db> ParseSpec<'db> for ast::generated::RefTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        Ok(Spec::new(
            sema.db,
            SpecKind::Ref(self.children.cast(sema.ast).to_spec(sema)?),
            self.into(),
            sema.current_scope,
        ))
    }
}

// Enum type

impl<'db> ParseSpec<'db> for ast::generated::EnumTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        let typ = self
            .children
            .cast(sema.ast)
            .elem_type
            .as_ref()
            .map(|elem| elem.cast(sema.ast).to_spec(sema))
            .transpose()?;

        let mut variants = vec![];
        for spec in &self.children.cast(sema.ast).children {
            let name =
                SpanIdent::from_node(sema.db, sema, spec.cast(sema.ast).value.cast(sema.ast))?;
            let value = spec
                .cast(sema.ast)
                .children
                .as_ref()
                .map(|v| v.cast(sema.ast).to_expr(sema))
                .transpose()?;

            variants.push(EnumVariant { name, value });
        }
        Ok(Spec::new(
            sema.db,
            SpecKind::Enum(Enum::new(sema.db, typ, variants)),
            self.into(),
            sema.current_scope,
        ))
    }
}

// Subrange type

impl<'db> ParseSpec<'db> for ast::generated::SubrangeTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>> {
        let spec = self.Type.cast(sema.ast).to_spec(sema)?;
        let range = self.range.cast(sema.ast);
        let lower = range
            .lower
            .cast(sema.ast)
            .children
            .cast(sema.ast)
            .to_expr(sema)?;
        let upper = range
            .upper
            .cast(sema.ast)
            .children
            .cast(sema.ast)
            .to_expr(sema)?;
        Ok(Spec::new(
            sema.db,
            SpecKind::Subrange(SubRange::new(sema.db, spec, lower, upper)),
            self.into(),
            sema.current_scope,
        ))
    }
}
