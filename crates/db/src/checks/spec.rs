use auto_lsp::default::db::File;

use crate::hir::{expression::Expr, variable::Spec};

impl<'db> Spec<'db> {
    fn check_init(&self, db: &'db dyn crate::BaseDatabase, file: File, init: &Expr<'db>) {
        match self {
            Spec::Target(_) => todo!(),
            Spec::Array(_) => todo!(),
            Spec::Subrange(_) => todo!(),
            Spec::Expr(_) => todo!(),
            Spec::Enum => todo!(),
            Spec::Struct => todo!(),
            Spec::Edge => todo!(),
            Spec::Bool => todo!(),
            Spec::Byte => todo!(),
            Spec::Word => todo!(),
            Spec::DWord => todo!(),
            Spec::LWord => todo!(),
            Spec::SInt => todo!(),
            Spec::USInt => todo!(),
            Spec::UInt => todo!(),
            Spec::Int => todo!(),
            Spec::DInt => todo!(),
            Spec::UDInt => todo!(),
            Spec::LInt => todo!(),
            Spec::ULInt => todo!(),
            Spec::Real => todo!(),
            Spec::LReal => todo!(),
            Spec::String => todo!(),
            Spec::WString => todo!(),
            Spec::Char => todo!(),
            Spec::WChar => todo!(),
            Spec::Date => todo!(),
            Spec::LDate => todo!(),
            Spec::Dt => todo!(),
            Spec::Ldt => todo!(),
            Spec::Time => todo!(),
            Spec::LTime => todo!(),
            Spec::Tod => todo!(),
            Spec::LTod => todo!(),
        }
    }
}