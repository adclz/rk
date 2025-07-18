use std::ops::Deref;

use crate::{
    hir::{data_type::DataType, namespace::PouDecl},
    ident::Ident,
    parser::{ParseInit, ParseSpec},
};
use auto_lsp::core::ast::AstNode;
use auto_lsp::{
    anyhow,
    default::db::{file::File, BaseDatabase},
};

pub trait ParseDataType<'db> {
    fn parse(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
        types: &mut Vec<PouDecl<'db>>,
    ) -> anyhow::Result<()>;
}

impl<'db> ParseDataType<'db> for ast::generated::DataTypeDecl {
    fn parse(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
        types: &mut Vec<PouDecl<'db>>,
    ) -> anyhow::Result<()> {
        type Spec = ast::generated::ArrayTypeSpec_EnumTypeSpec_RefTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec_SubrangeTypeSpec;

        for child in &self.children {
            let name = Ident::from_node(db, file, child.name.deref())?;

            let spec = match child.spec.deref() {
                Spec::ArrayTypeSpec(a) => a.to_spec(db, file),
                Spec::EnumTypeSpec(a) => a.to_spec(db, file),
                Spec::SimpleTypeSpec(a) => a.to_spec(db, file),
                Spec::StrTypeSpec(a) => a.to_spec(db, file),
                Spec::StructTypeSpec(a) => a.to_spec(db, file),
                Spec::SubrangeTypeSpec(a) => a.to_spec(db, file),
                Spec::RefTypeSpec(a) => a.to_spec(db, file),
            }?;

            type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

            let init = match child.init.as_deref() {
                Some(Init::ArrayTypeInit(a)) => Some(a.to_init(db, file)?),
                Some(Init::SimpleTypeInit(a)) => Some(a.to_init(db, file)?),
                Some(Init::StructTypeInit(a)) => Some(a.to_init(db, file)?),
                None => None,
            };

            types.push(PouDecl::new(
                db,
                file,
                crate::hir::namespace::Pou::DataType(DataType::new(db, spec, init)),
                child.get_span(),
                name,
                child.name.get_span(),
                self.get_id()
            ))
        }

        Ok(())
    }
}
