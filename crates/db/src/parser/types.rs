use auto_lsp::{
    anyhow::{self},
    core::ast::AstNode,
};

use crate::{
    hir::{
        expressions::{
            expression::{
                InitExpr, InitExprKind, MultibitsPart, Numeric, NumericKind
            },
            spec::{Enum, EnumVariant, Spec, SpecKind, Struct, StructElement, SubRange},
        },
        interned::{identifier::Ident, namespace::NamespaceAccess},
    },
    parser::{
        expression::{ParseExpr, ParseExpression, ParseVariableAccess},
        semantic_index::SemanticIndexBuilder,
        ParseSpec, ParseSpecInit, SpecInitResult,
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
            sema.file,
        ))
    }
}

// Simple type

impl<'db> ParseSpec<'db> for ast::generated::SimpleTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        self.children.cast(&sema.ast).to_spec(sema)
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
            AstSpec::BitStrTypeName(str) => match str.children.cast(&sema.ast) {
                ast::generated::BoolName_MultibitsTypeName::BoolName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Bool,
                    sema.current_scope,
                    sema.file,
                ),
                ast::generated::BoolName_MultibitsTypeName::MultibitsTypeName(a) => {
                    match a.children.cast(&sema.ast) {
                        ast::generated::ByteName_DwordName_LwordName_WordName::ByteName(_) => {
                            Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::Byte,
                                sema.current_scope,
                                sema.file,
                            )
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::WordName(_) => {
                            Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::Word,
                                sema.current_scope,
                                sema.file,
                            )
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::DwordName(_) => {
                            Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::DWord,
                                sema.current_scope,
                                sema.file,
                            )
                        }
                        ast::generated::ByteName_DwordName_LwordName_WordName::LwordName(_) => {
                            Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::LWord,
                                sema.current_scope,
                                sema.file,
                            )
                        }
                    }
                }
            },
            AstSpec::NumericTypeName(numeric_type_name) => {
                match numeric_type_name.children.cast(&sema.ast) {
                    ast::generated::IntTypeName_RealTypeName::IntTypeName(int) => {
                        int.to_spec(sema)?
                    }
                    ast::generated::IntTypeName_RealTypeName::RealTypeName(real) => {
                        match real.children.cast(&sema.ast) {
                            ast::generated::LrealName_RealName::RealName(_) => Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::Real,
                                sema.current_scope,
                                sema.file,
                            ),
                            ast::generated::LrealName_RealName::LrealName(_) => Spec::new(
                                sema.db,
                                self.get_span(),
                                SpecKind::LReal,
                                sema.current_scope,
                                sema.file,
                            ),
                        }
                    }
                }
            }
            AstSpec::AnyDateTypeName(date_type_name) => match date_type_name {
                ast::generated::AnyDateTypeName::DateTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Date,
                    sema.current_scope,
                    sema.file,
                ),
                ast::generated::AnyDateTypeName::LDateTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::LDate,
                    sema.current_scope,
                    sema.file,
                ),
            },
            AstSpec::AnyTimeTypeName(time_type_name) => match time_type_name {
                ast::generated::AnyTimeTypeName::TimeTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Time,
                    sema.current_scope,
                    sema.file,
                ),
                ast::generated::AnyTimeTypeName::LTimeTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::LTime,
                    sema.current_scope,
                    sema.file,
                ),
            },
            AstSpec::AnyTodTypeName(tod_type_name) => match tod_type_name {
                ast::generated::AnyTodTypeName::TodTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Tod,
                    sema.current_scope,
                    sema.file,
                ),
                ast::generated::AnyTodTypeName::LtodTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::LTod,
                    sema.current_scope,
                    sema.file,
                ),
            },
            AstSpec::AnyDtTypeName(dt_type_name) => match dt_type_name {
                ast::generated::AnyDtTypeName::DtTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Dt,
                    sema.current_scope,
                    sema.file,
                ),
                ast::generated::AnyDtTypeName::LDtTypeName(_) => Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Ldt,
                    sema.current_scope,
                    sema.file,
                ),
            },
        })
    }
}

impl<'db> ParseSpec<'db> for ast::generated::IntTypeName {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        Ok(match self.children.cast(&sema.ast) {
            ast::generated::SignIntTypeName_UnsignIntTypeName::SignIntTypeName(int) => {
                match int.children.cast(&sema.ast) {
                    ast::generated::DintName_IntName_LintName_SintName::SintName(_) => Spec::new(
                        sema.db,
                        self.get_span(),
                        SpecKind::SInt,
                        sema.current_scope,
                        sema.file,
                    ),
                    ast::generated::DintName_IntName_LintName_SintName::IntName(_) => Spec::new(
                        sema.db,
                        self.get_span(),
                        SpecKind::Int,
                        sema.current_scope,
                        sema.file,
                    ),
                    ast::generated::DintName_IntName_LintName_SintName::DintName(_) => Spec::new(
                        sema.db,
                        self.get_span(),
                        SpecKind::DInt,
                        sema.current_scope,
                        sema.file,
                    ),
                    ast::generated::DintName_IntName_LintName_SintName::LintName(_) => Spec::new(
                        sema.db,
                        self.get_span(),
                        SpecKind::LInt,
                        sema.current_scope,
                        sema.file,
                    ),
                }
            }
            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(uint) => {
                match uint.children.cast(&sema.ast) {
                    ast::generated::UdintName_UintName_UlintName_UsintName::UsintName(_) => {
                        Spec::new(
                            sema.db,
                            self.get_span(),
                            SpecKind::USInt,
                            sema.current_scope,
                            sema.file,
                        )
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UintName(_) => {
                        Spec::new(
                            sema.db,
                            self.get_span(),
                            SpecKind::UInt,
                            sema.current_scope,
                            sema.file,
                        )
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UdintName(_) => {
                        Spec::new(
                            sema.db,
                            self.get_span(),
                            SpecKind::UDInt,
                            sema.current_scope,
                            sema.file,
                        )
                    }
                    ast::generated::UdintName_UintName_UlintName_UsintName::UlintName(_) => {
                        Spec::new(
                            sema.db,
                            self.get_span(),
                            SpecKind::ULInt,
                            sema.current_scope,
                            sema.file,
                        )
                    }
                }
            }
        })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::SimpleTypeInit {
    type Output = InitExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<InitExpr<'db>> {
        Ok(InitExpr{
            span: self.get_span(),
            kind: InitExprKind::ConstantExpr(self.children.cast(&sema.ast).children.cast(&sema.ast).to_expr(sema)?)
        }) 
    }
}

// String type

impl<'db> ParseSpec<'db> for ast::generated::StrTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        type AstSpec = ast::generated::DByteStrSpec_DChar_SByteStrSpec_SChar;
        Ok(match self.children.cast(&sema.ast) {
            AstSpec::DByteStrSpec(_) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::WString,
                sema.current_scope,
                sema.file,
            ),

            AstSpec::SByteStrSpec(_) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::String,
                sema.current_scope,
                sema.file,
            ),

            AstSpec::DChar(_) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::WChar,
                sema.current_scope,
                sema.file,
            ),

            AstSpec::SChar(_) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::Char,
                sema.current_scope,
                sema.file,
            ),
        })
    }
}

// Array type

impl<'db> ParseSpec<'db> for ast::generated::ArrayTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        match self.Type.cast(&sema.ast) {
            ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                elem_type_name.to_spec(sema)
            }
            ast::generated::DataTypeAccess::NamespaceAccess(target) => target.to_spec(sema),
        }
    }
}

impl<'db> ParseExpr<'db> for ast::generated::ArrayTypeInit {
    type Output = InitExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        let values = self
            .children
            .iter()
            .map(|elem| elem.cast(&sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(InitExpr { span: self.get_span(), kind: InitExprKind::ArrayInit { values } })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::StructTypeInit {
    type Output = InitExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        let values = self
            .children
            .iter()
            .map(|elem| elem.cast(&sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(InitExpr { span: self.get_span(), kind: InitExprKind::ArrayInit { values } })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::InitElem {
    type Output = InitExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        type InitElem = ast::generated::ERRFuncCallInInit_ArrayIndexElem_ArrayInit_ConstantExpr_StructElem_StructInit;
        match self.children.cast(&sema.ast) {
            InitElem::ArrayIndexElem(array_type_init) => {
                array_type_init.parse(sema)
            }
            InitElem::ArrayInit(expr) => {
                expr.parse(sema)
            }
            InitElem::StructElem(struct_elem_init) => {
                struct_elem_init.parse(sema)
            },
            InitElem::StructInit(struct_type_init) => {
                struct_type_init.parse(sema)
            }
            InitElem::ConstantExpr(expr) => { 
                Ok(InitExpr {
                    span: self.get_span(),
                    kind: InitExprKind::ConstantExpr(expr.children.cast(&sema.ast).to_expr(sema)?),
                })
            },
            InitElem::ERRFuncCallInInit(err) => {
                todo!()
            }
        }
    }
}

impl<'db> ParseExpr<'db> for ast::generated::ArrayInit {
    type Output = InitExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        let values = self
            .values
            .cast(&sema.ast) 
            .children
            .iter()
            .map(|elem| elem.cast(&sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<_>>>()?;
 
        Ok(InitExpr { span: self.get_span(), kind: InitExprKind::ArrayInit { values } })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::ArrayIndexElem {
    type Output = InitExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        let index = Numeric::new(
            sema.db,
            Ident::from_node(sema.db, sema.file, self.index.cast(&sema.ast))?,
            NumericKind::Signed,
        ); 

        let values = self
            .values
            .cast(&sema.ast)
            .children
            .iter()
            .map(|elem| elem.cast(&sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(InitExpr { span: self.get_span(), kind: InitExprKind::ArrayIndexedElement { index, values } })
    }
} 

impl<'db> ParseExpr<'db> for ast::generated::StructInit {
    type Output = InitExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        let values = self
            .children
            .iter()
            .map(|elem| elem.cast(&sema.ast).parse(sema))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(InitExpr { span: self.get_span(), kind: InitExprKind::StructInit { values } })
    }
}

impl<'db> ParseExpr<'db> for ast::generated::StructElem {
    type Output = InitExpr<'db>;

    fn parse(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Self::Output> {
        let name = Ident::from_node(sema.db, sema.file, self.name.cast(&sema.ast))?;
        let value = Box::new(self.value.cast(&sema.ast).parse(sema)?);

        Ok(InitExpr { span: self.get_span(), kind: InitExprKind::StructElement { name, value }})
    }
}

// Array conformand

impl<'db> ParseSpec<'db> for ast::generated::ArrayConformand {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        Ok(Spec::new(
            sema.db,
            self.get_span(),
            SpecKind::ArrayConformand(self.children.cast(&sema.ast).to_spec(sema)?),
            sema.current_scope,
            sema.file,
        ))
    }
}

impl<'db> ParseSpec<'db> for ast::generated::StructTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {

        let mut elements = vec![];

        for elem in &self.children {
             type Spec = ast::generated::ArrayTypeSpec_EnumTypeSpec_SimpleTypeSpec_StructTypeSpec_SubrangeTypeSpec;

            let spec = match elem.cast(&sema.ast).spec.cast(&sema.ast) {
                Spec::ArrayTypeSpec(elem) => elem.to_spec(sema)?,
                Spec::SimpleTypeSpec(init) => init.to_spec(sema)?,
                Spec::EnumTypeSpec(en) => en.to_spec(sema)?,
                Spec::SubrangeTypeSpec(sub) => sub.to_spec(sema)?,
                Spec::StructTypeSpec(st) => st.to_spec(sema)?,
            };

            type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

            let init = if let Some(init) = elem.cast(&sema.ast).init.as_ref().map(|i| i.cast(&sema.ast)) {
                match init {
                    Init::ArrayTypeInit(array_type_init) => Some(array_type_init.parse(sema)?),
                    Init::SimpleTypeInit(simple_type_init) => Some(simple_type_init.parse(sema)?),
                    Init::StructTypeInit(struct_type_init) => Some(struct_type_init.parse(sema)?),
                }
            } else {
                None
            };

            let name = Ident::from_node(sema.db, sema.file, elem.cast(&sema.ast).name.cast(&sema.ast))?;

            let (located, multibits) = if let Some(attrs) = &elem.cast(&sema.ast).attributes {
                let located = attrs.cast(&sema.ast).located.cast(&sema.ast).children.cast(&sema.ast).to_access(sema).ok();
                let multibits = attrs
                    .cast(&sema.ast)
                    .multibits
                    .as_ref()
                    .map(|mb| mb.cast(&sema.ast).to_multibits(sema))
                    .transpose()?;
                (located, multibits)
            } else {
                (None, None)
            };

            elements.push(StructElement { 
                name,
                spec,
                init,
                located,
                multibits,
            });
        }

        Ok(Spec::new(
            sema.db,
            self.get_span(),
            SpecKind::Struct(Struct {
                elements,
                overlap: self.overlap.is_some(),
            }),
            sema.current_scope,
            sema.file,
        ))
    }
}

pub trait ParseMultiBits<'db> {
    fn to_multibits(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<MultibitsPart>;
}

impl<'db> ParseMultiBits<'db> for ast::generated::MultibitPartAccess {
    fn to_multibits(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<MultibitsPart> {
        let offset = Numeric::new(
            sema.db,
            Ident::from_node(sema.db, sema.file, self.path.cast(&sema.ast).children.cast(&sema.ast))?,
            NumericKind::Signed,
        );

        match &self.path.cast(&sema.ast).access {
            Some(access) => Ok(MultibitsPart::AccessOffset {
                offset,
                access: Ident::from_node(sema.db, sema.file, access.cast(&sema.ast))?,
            }),
            None => Ok(MultibitsPart::Offset(offset)),
        }
    }
}

// RefSpec

impl<'db> ParseSpecInit<'db> for ast::generated::RefSpec {
    fn to_spec_init(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        Ok(SpecInitResult { spec: self.children.cast(&sema.ast).to_spec(sema)?, init: None })
    }
}

impl<'db> ParseSpec<'db> for ast::generated::RefTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        Ok(Spec::new(
            sema.db,
            self.get_span(),
            SpecKind::Ref(self.children.cast(&sema.ast).to_spec(sema)?),
            sema.current_scope,
            sema.file,
        ))
    }
}

// Enum type

impl<'db> ParseSpec<'db> for ast::generated::EnumTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        let typ = self.children.cast(&sema.ast).elem_type
            .as_ref()
            .map(|elem| elem.cast(&sema.ast).to_spec(sema))
            .transpose()?;
        
        let mut variants = vec![];
        for spec in &self.children.cast(&sema.ast).children {
            let name = Ident::from_node(sema.db, sema.file, spec.cast(&sema.ast).value.cast(&sema.ast))?;
            let value = spec.cast(&sema.ast).children
                .as_ref()
                .map(|v| v.cast(&sema.ast).to_expr(sema))
                .transpose()?;

            variants.push(EnumVariant {
                name,
                value,
            });
        }
        Ok(Spec::new(sema.db, self.get_span(), 
        SpecKind::Enum(Enum {
            typ,
            variants
        }),
         sema.current_scope, sema.file))
    }
}

// Subrange type

impl<'db> ParseSpec<'db> for ast::generated::SubrangeTypeSpec {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>> {
        let spec = self.Type.cast(&sema.ast).to_spec(sema)?;
        let range = self.range.cast(&sema.ast);
        let lower = range.lower.cast(&sema.ast).children.cast(&sema.ast).to_expr(sema)?;
        let upper = range.upper.cast(&sema.ast).children.cast(&sema.ast).to_expr(sema)?;
        Ok(Spec::new(
            sema.db,
            self.get_span(),
            SpecKind::Subrange(SubRange {
                _type: Box::new(spec),
                lower,
                upper,
            }),
            sema.current_scope,
            sema.file,
        ))
    }
}
