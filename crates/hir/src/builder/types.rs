use auto_lsp::anyhow::{self};
use auto_lsp::core::ast::AstNode;

use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::expressions::spec::Array;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use crate::{
    builder::{
        Parse, ParseSpec, ParseSpecInit, SpecInitResult, expression::ParseVariableAccess,
        semantic_index::SemanticIndexBuilder,
    },
    check::errors::ToIdeDiagnostic,
    hir_def::{
        expressions::{
            expression::{
                Elementary, ExprKind, InitExpr, InitExprKind, Integer, IntegerKind, MultibitsPart,
                PrimaryExpr,
            },
            spec::{ElementarySpec, Enum, EnumVariant, Spec, SpecKind, Struct, SubRange},
        },
        interned::identifier::Ident,
    },
};
use ide_diagnostic::IdeDiagnostic;

// Target

impl<'db> ParseSpecInit<'db> for ast::generated::NamespaceAccess {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, IdeDiagnostic> {
        Ok(SpecInitResult::new(self.to_spec(sema)?, None))
    }
}

impl<'db> ParseSpec<'db> for ast::generated::NamespaceAccess {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        Ok(sema.new_spec(
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
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        self.children.cast(sema.ast).to_spec(sema)
    }
}

impl<'db> ParseSpec<'db> for ast::generated::DataTypeAccess {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        match self {
            ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                elem_type_name.to_spec(sema)
            }
            ast::generated::DataTypeAccess::NamespaceAccess(target) => target.to_spec(sema),
            ast::generated::DataTypeAccess::UserTypeRef(user_ref) => {
                // `Counter<INT>` - build the path spec and attach the parsed
                // type arguments. Validation against the referenced FB's
                // implicit ANY_* param list happens later (arg count match +
                // bound conformance); at this layer we only record what the
                // user wrote.
                let path_ast = user_ref.path.cast(sema.ast);
                let mut path = SpanNamespaceAccess::from_ast(sema.db, sema, path_ast)?;
                let args_ast = user_ref.type_args.cast(sema.ast);
                let args: Vec<Spec<'db>> = args_ast
                    .arg
                    .iter()
                    .filter_map(|a| {
                        let s = a.cast(sema.ast).to_spec(sema);
                        sema.try_parse(s)
                    })
                    .collect();
                path.type_args = args;
                Ok(sema.new_spec(SpecKind::Target(path), user_ref.into(), sema.current_scope))
            }
            ast::generated::DataTypeAccess::IntoSpec(into) => {
                let ident = crate::hir_def::interned::identifier::SpanIdent::from_node(
                    sema.db,
                    sema,
                    into.Ref.cast(sema.ast),
                )?;
                Ok(sema.new_spec(SpecKind::Into(ident), into.into(), sema.current_scope))
            }
        }
    }
}

impl<'db> ParseSpec<'db> for ast::generated::ElemTypeName {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        type AstSpec = ast::generated::ElemTypeName;
        Ok(match self {
            AstSpec::BitStrTypeName(str) => match str.children.cast(sema.ast) {
                ast::generated::BoolName_MultibitsTypeName::BoolName(_) => sema.new_spec(
                    SpecKind::Simple(ElementarySpec::Bool),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::BoolName_MultibitsTypeName::MultibitsTypeName(a) => {
                    match a.children.cast(sema.ast) {
                        ast::generated::ByteName_DwordName_LwordName_WordName::ByteName(_) => sema
                            .new_spec(
                                SpecKind::Simple(ElementarySpec::Byte),
                                self.into(),
                                sema.current_scope,
                            ),
                        ast::generated::ByteName_DwordName_LwordName_WordName::WordName(_) => sema
                            .new_spec(
                                SpecKind::Simple(ElementarySpec::Word),
                                self.into(),
                                sema.current_scope,
                            ),
                        ast::generated::ByteName_DwordName_LwordName_WordName::DwordName(_) => sema
                            .new_spec(
                                SpecKind::Simple(ElementarySpec::DWord),
                                self.into(),
                                sema.current_scope,
                            ),
                        ast::generated::ByteName_DwordName_LwordName_WordName::LwordName(_) => sema
                            .new_spec(
                                SpecKind::Simple(ElementarySpec::LWord),
                                self.into(),
                                sema.current_scope,
                            ),
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
                            ast::generated::LrealName_RealName::RealName(_) => sema.new_spec(
                                SpecKind::Simple(ElementarySpec::Real),
                                self.into(),
                                sema.current_scope,
                            ),
                            ast::generated::LrealName_RealName::LrealName(_) => sema.new_spec(
                                SpecKind::Simple(ElementarySpec::LReal),
                                self.into(),
                                sema.current_scope,
                            ),
                        }
                    }
                }
            }
            AstSpec::AnyDateTypeName(date_type_name) => match date_type_name {
                ast::generated::AnyDateTypeName::DateTypeName(_) => sema.new_spec(
                    SpecKind::Simple(ElementarySpec::Date),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::AnyDateTypeName::LDateTypeName(_) => sema.new_spec(
                    SpecKind::Simple(ElementarySpec::LDate),
                    self.into(),
                    sema.current_scope,
                ),
            },
            AstSpec::AnyTimeTypeName(time_type_name) => match time_type_name {
                ast::generated::AnyTimeTypeName::TimeTypeName(_) => sema.new_spec(
                    SpecKind::Simple(ElementarySpec::Time),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::AnyTimeTypeName::LTimeTypeName(_) => sema.new_spec(
                    SpecKind::Simple(ElementarySpec::LTime),
                    self.into(),
                    sema.current_scope,
                ),
            },
            AstSpec::AnyTodTypeName(tod_type_name) => match tod_type_name {
                ast::generated::AnyTodTypeName::TodTypeName(_) => sema.new_spec(
                    SpecKind::Simple(ElementarySpec::Tod),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::AnyTodTypeName::LtodTypeName(_) => sema.new_spec(
                    SpecKind::Simple(ElementarySpec::LTod),
                    self.into(),
                    sema.current_scope,
                ),
            },
            AstSpec::AnyDtTypeName(dt_type_name) => match dt_type_name {
                ast::generated::AnyDtTypeName::DtTypeName(_) => sema.new_spec(
                    SpecKind::Simple(ElementarySpec::DateAndTime),
                    self.into(),
                    sema.current_scope,
                ),
                ast::generated::AnyDtTypeName::LDtTypeName(_) => sema.new_spec(
                    SpecKind::Simple(ElementarySpec::LDateTime),
                    self.into(),
                    sema.current_scope,
                ),
            },
            AstSpec::StringTypeName(string_type_name) => {
                match string_type_name.children.cast(sema.ast) {
                    ast::generated::CharName_StringName::StringName(string_name) => {
                        match &string_name.children {
                            Some(unsigned_int) => {
                                let unsigned_int = unsigned_int.cast(sema.ast);
                                let ident = Ident::from_node(sema.db, sema.file, unsigned_int)?;
                                let integer = Integer::new(sema.db, ident, IntegerKind::Signed);
                                let length_expr = sema.new_expr(
                                    ExprKind::PrimaryExpr(PrimaryExpr::Literal(
                                        Elementary::InferInteger(integer),
                                    )),
                                    unsigned_int.into(),
                                    sema.current_scope,
                                );
                                sema.new_spec(
                                    SpecKind::SizedString(length_expr),
                                    self.into(),
                                    sema.current_scope,
                                )
                            }
                            None => sema.new_spec(
                                SpecKind::Simple(ElementarySpec::String),
                                self.into(),
                                sema.current_scope,
                            ),
                        }
                    }
                    ast::generated::CharName_StringName::CharName(_) => sema.new_spec(
                        SpecKind::Simple(ElementarySpec::Char),
                        self.into(),
                        sema.current_scope,
                    ),
                }
            }
            AstSpec::AnyTypeName(any) => {
                type AnyKind = ast::generated::AnyBitName_AnyDateName_AnyDurationName_AnyElementaryName_AnyIntName_AnyMagnitudeName_AnyName_AnyNumName_AnyRealName_AnySignedName_AnyUnsignedName;
                let spec = match any.children.cast(sema.ast) {
                    AnyKind::AnyName(_) => ElementarySpec::Any,
                    AnyKind::AnyNumName(_) => ElementarySpec::AnyNum,
                    AnyKind::AnyIntName(_) => ElementarySpec::AnyInt,
                    AnyKind::AnyRealName(_) => ElementarySpec::AnyReal,
                    AnyKind::AnyBitName(_) => ElementarySpec::AnyBit,
                    AnyKind::AnyElementaryName(_) => ElementarySpec::AnyElementary,
                    AnyKind::AnyMagnitudeName(_) => ElementarySpec::AnyMagnitude,
                    AnyKind::AnyDateName(_) => ElementarySpec::AnyDate,
                    AnyKind::AnyDurationName(_) => ElementarySpec::AnyDuration,
                    AnyKind::AnySignedName(_) => ElementarySpec::AnySigned,
                    AnyKind::AnyUnsignedName(_) => ElementarySpec::AnyUnsigned,
                };
                sema.new_spec(SpecKind::Simple(spec), self.into(), sema.current_scope)
            }
        })
    }
}

impl<'db> ParseSpec<'db> for ast::generated::IntTypeName {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        Ok(match self.children.cast(sema.ast) {
            ast::generated::SignIntTypeName_UnsignIntTypeName::SignIntTypeName(int) => {
                match int.children.cast(sema.ast) {
                    ast::generated::DintName_IntName_LintName_SintName::SintName(_) => sema
                        .new_spec(
                            SpecKind::Simple(ElementarySpec::SInt),
                            self.into(),
                            sema.current_scope,
                        ),
                    ast::generated::DintName_IntName_LintName_SintName::IntName(_) => sema
                        .new_spec(
                            SpecKind::Simple(ElementarySpec::Int),
                            self.into(),
                            sema.current_scope,
                        ),
                    ast::generated::DintName_IntName_LintName_SintName::DintName(_) => sema
                        .new_spec(
                            SpecKind::Simple(ElementarySpec::DInt),
                            self.into(),
                            sema.current_scope,
                        ),
                    ast::generated::DintName_IntName_LintName_SintName::LintName(_) => sema
                        .new_spec(
                            SpecKind::Simple(ElementarySpec::LInt),
                            self.into(),
                            sema.current_scope,
                        ),
                }
            }
            ast::generated::SignIntTypeName_UnsignIntTypeName::UnsignIntTypeName(uint) => {
                match uint.children.cast(sema.ast) {
                    ast::generated::UdintName_UintName_UlintName_UsintName::UsintName(_) => sema
                        .new_spec(
                            SpecKind::Simple(ElementarySpec::USInt),
                            self.into(),
                            sema.current_scope,
                        ),
                    ast::generated::UdintName_UintName_UlintName_UsintName::UintName(_) => sema
                        .new_spec(
                            SpecKind::Simple(ElementarySpec::UInt),
                            self.into(),
                            sema.current_scope,
                        ),
                    ast::generated::UdintName_UintName_UlintName_UsintName::UdintName(_) => sema
                        .new_spec(
                            SpecKind::Simple(ElementarySpec::UDInt),
                            self.into(),
                            sema.current_scope,
                        ),
                    ast::generated::UdintName_UintName_UlintName_UsintName::UlintName(_) => sema
                        .new_spec(
                            SpecKind::Simple(ElementarySpec::ULInt),
                            self.into(),
                            sema.current_scope,
                        ),
                }
            }
        })
    }
}

impl<'db> Parse<'db> for ast::generated::SimpleTypeInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<InitExpr<'db>, IdeDiagnostic> {
        let kind = InitExprKind::ConstantExpr(
            self.children
                .cast(sema.ast)
                .children
                .cast(sema.ast)
                .parse(sema)?,
        );
        Ok(sema.new_init_expr(kind, self.into(), sema.current_scope))
    }
}

// Array type

impl<'db> ParseSpec<'db> for ast::generated::ArrayTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        let mut ranges = vec![];

        for range in &self.ranges.cast(sema.ast).children {
            let r = range
                .cast(sema.ast)
                .lower
                .cast(sema.ast)
                .children
                .cast(sema.ast)
                .parse(sema);
            let Some(lower) = sema.try_parse(r) else {
                continue;
            };
            let r = range
                .cast(sema.ast)
                .upper
                .cast(sema.ast)
                .children
                .cast(sema.ast)
                .parse(sema);
            let Some(upper) = sema.try_parse(r) else {
                continue;
            };
            ranges.push((lower, upper))
        }

        let kind = match self.spec.cast(sema.ast) {
            ast::generated::RefTypeSpec_SimpleTypeSpec::SimpleTypeSpec(simple) => {
                simple.children.cast(sema.ast).to_spec(sema)
            }
            ast::generated::RefTypeSpec_SimpleTypeSpec::RefTypeSpec(ref_spec) => {
                let mut target_type = ref_spec.children.cast(sema.ast).to_spec(sema)?;
                for _ in ref_spec.ref_count.iter() {
                    target_type =
                        sema.new_spec(SpecKind::Ref(target_type), self.into(), sema.current_scope);
                }
                Ok(target_type)
            }
        }?;

        Ok(sema.new_spec(
            SpecKind::Array(Array::new(sema.db, ranges, kind)),
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> Parse<'db> for ast::generated::ArrayTypeInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, IdeDiagnostic> {
        let values = self
            .children
            .iter()
            .filter_map(|elem| {
                let r = elem.cast(sema.ast).parse(sema);
                sema.try_parse(r)
            })
            .collect();

        Ok(sema.new_init_expr(
            InitExprKind::ArrayInit { values },
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> Parse<'db> for ast::generated::StructTypeInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, IdeDiagnostic> {
        let values = self
            .children
            .iter()
            .filter_map(|elem| {
                let r = elem.cast(sema.ast).parse(sema);
                sema.try_parse(r)
            })
            .collect();

        Ok(sema.new_init_expr(
            InitExprKind::StructInit { values },
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> Parse<'db> for ast::generated::InitElem {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, IdeDiagnostic> {
        type InitElem = ast::generated::ERRFuncCallInInit_ArrayIndexElem_ArrayInit_ConstantExpr_StructElem_StructInit;
        match self.children.cast(sema.ast) {
            InitElem::ArrayIndexElem(array_type_init) => array_type_init.parse(sema),
            InitElem::ArrayInit(expr) => expr.parse(sema),
            InitElem::StructElem(struct_elem_init) => struct_elem_init.parse(sema),
            InitElem::StructInit(struct_type_init) => struct_type_init.parse(sema),
            InitElem::ConstantExpr(expr) => {
                let kind = InitExprKind::ConstantExpr(expr.children.cast(sema.ast).parse(sema)?);
                Ok(sema.new_init_expr(kind, self.into(), sema.current_scope))
            }
            InitElem::ERRFuncCallInInit(err) => Err(SyntaxError::FunctionCallInInitExpression(
                err.get_range().to_owned(),
            )
            .to_diagnostic(sema.db, sema.file)),
        }
    }
}

impl<'db> Parse<'db> for ast::generated::ArrayInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, IdeDiagnostic> {
        let values = self
            .values
            .cast(sema.ast)
            .children
            .iter()
            .filter_map(|elem| {
                let r = elem.cast(sema.ast).parse(sema);
                sema.try_parse(r)
            })
            .collect();

        Ok(sema.new_init_expr(
            InitExprKind::ArrayInit { values },
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> Parse<'db> for ast::generated::ArrayIndexElem {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, IdeDiagnostic> {
        let index = SpanIdent::from_node(sema.db, sema, self.index.cast(sema.ast))?;

        let values = self
            .values
            .cast(sema.ast)
            .children
            .iter()
            .filter_map(|elem| {
                let r = elem.cast(sema.ast).parse(sema);
                sema.try_parse(r)
            })
            .collect();

        let kind = InitExprKind::ArrayIndexedElement {
            size: index,
            values,
        };
        Ok(sema.new_init_expr(kind, self.into(), sema.current_scope))
    }
}

impl<'db> Parse<'db> for ast::generated::StructInit {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, IdeDiagnostic> {
        let values = self
            .children
            .iter()
            .filter_map(|elem| {
                let r = elem.cast(sema.ast).parse(sema);
                sema.try_parse(r)
            })
            .collect();

        Ok(sema.new_init_expr(
            InitExprKind::StructInit { values },
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> Parse<'db> for ast::generated::StructElem {
    type Output = InitExpr<'db>;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, IdeDiagnostic> {
        let name = SpanIdent::from_node(sema.db, sema, self.name.cast(sema.ast))?;
        let value = Box::new(self.value.cast(sema.ast).parse(sema)?);
        let kind = InitExprKind::StructElement { name, value };
        Ok(sema.new_init_expr(kind, self.into(), sema.current_scope))
    }
}

// Array conformand

impl<'db> ParseSpec<'db> for ast::generated::ArrayConformand {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        Err(
            SyntaxError::ArrayConformandNotSupported(self.get_range().to_owned())
                .to_diagnostic(sema.db, sema.file),
        )
    }
}

impl<'db> ParseSpec<'db> for ast::generated::StructTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        let mut elements = vec![];

        for elem in &self.children {
            type Spec = ast::generated::ArrayTypeSpec_EnumTypeSpec_RefTypeSpec_SimpleTypeSpec_StructTypeSpec_SubrangeTypeSpec;

            let r = match elem.cast(sema.ast).spec.cast(sema.ast) {
                Spec::ArrayTypeSpec(elem) => elem.to_spec(sema),
                Spec::SimpleTypeSpec(init) => init.to_spec(sema),
                Spec::EnumTypeSpec(en) => en.to_spec(sema),
                Spec::SubrangeTypeSpec(sub) => sub.to_spec(sema),
                Spec::StructTypeSpec(st) => st.to_spec(sema),
                Spec::RefTypeSpec(rf) => rf.to_spec(sema),
            };
            let Some(spec) = sema.try_parse(r) else {
                continue;
            };

            type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

            let init =
                if let Some(init) = elem.cast(sema.ast).init.as_ref().map(|i| i.cast(sema.ast)) {
                    let r = match init {
                        Init::ArrayTypeInit(array_type_init) => array_type_init.parse(sema),
                        Init::SimpleTypeInit(simple_type_init) => simple_type_init.parse(sema),
                        Init::StructTypeInit(struct_type_init) => struct_type_init.parse(sema),
                    };
                    sema.try_parse(r)
                } else {
                    None
                };

            let r = Ident::from_node(sema.db, sema.file, elem.cast(sema.ast).name.cast(sema.ast));
            let Some(name) = sema.try_parse(r) else {
                continue;
            };

            let (located, multibits) = if let Some(attrs) = &elem.cast(sema.ast).attributes {
                let r = attrs
                    .cast(sema.ast)
                    .located
                    .cast(sema.ast)
                    .children
                    .cast(sema.ast)
                    .to_access(sema);
                let located = sema.try_parse(r);
                let r = attrs
                    .cast(sema.ast)
                    .multibits
                    .as_ref()
                    .map(|mb| mb.cast(sema.ast).parse(sema));
                let multibits = r.and_then(|r| sema.try_parse(r));
                (located, multibits)
            } else {
                (None, None)
            };

            elements.push(sema.new_struct_element(
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

        Ok(sema.new_spec(
            SpecKind::Struct(Struct::new(sema.db, self.overlap.is_some(), elements)),
            self.into(),
            sema.current_scope,
        ))
    }
}

impl<'db> Parse<'db> for ast::generated::MultibitPartAccess {
    type Output = MultibitsPart;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<MultibitsPart, IdeDiagnostic> {
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
    ) -> anyhow::Result<SpecInitResult<'db>, IdeDiagnostic> {
        Ok(SpecInitResult {
            spec: self.children.cast(sema.ast).to_spec(sema)?,
            init: None,
        })
    }
}

impl<'db> ParseSpec<'db> for ast::generated::ArrayTypeSpec_DataTypeAccess {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        match self {
            ast::generated::ArrayTypeSpec_DataTypeAccess::DataTypeAccess(dt) => dt.to_spec(sema),
            ast::generated::ArrayTypeSpec_DataTypeAccess::ArrayTypeSpec(arr) => arr.to_spec(sema),
        }
    }
}

impl<'db> ParseSpec<'db> for ast::generated::RefTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        let mut target_type = self.children.cast(sema.ast).to_spec(sema)?;

        // for each ref count, we need to create a new spec that wraps the previous one in a RefSpec
        for _ in self.ref_count.iter() {
            target_type =
                sema.new_spec(SpecKind::Ref(target_type), self.into(), sema.current_scope);
        }

        Ok(target_type)
    }
}

// Enum type

impl<'db> ParseSpec<'db> for ast::generated::EnumTypeSpec {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        let typ = self
            .children
            .cast(sema.ast)
            .elem_type
            .as_ref()
            .map(|elem| elem.cast(sema.ast).to_spec(sema))
            .transpose()?;

        let mut variants = vec![];
        for spec in &self.children.cast(sema.ast).children {
            let r = SpanIdent::from_node(sema.db, sema, spec.cast(sema.ast).value.cast(sema.ast));
            let Some(name) = sema.try_parse(r) else {
                continue;
            };
            let value = spec
                .cast(sema.ast)
                .children
                .as_ref()
                .map(|v| v.cast(sema.ast).parse(sema));
            let value = value.and_then(|r| sema.try_parse(r));

            variants.push(EnumVariant { name, value });
        }
        Ok(sema.new_spec(
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
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic> {
        let spec = self.Type.cast(sema.ast).to_spec(sema)?;
        let range = self.range.cast(sema.ast);
        let lower = range
            .lower
            .cast(sema.ast)
            .children
            .cast(sema.ast)
            .parse(sema)?;
        let upper = range
            .upper
            .cast(sema.ast)
            .children
            .cast(sema.ast)
            .parse(sema)?;
        Ok(sema.new_spec(
            SpecKind::Subrange(SubRange::new(sema.db, spec, lower, upper)),
            self.into(),
            sema.current_scope,
        ))
    }
}
