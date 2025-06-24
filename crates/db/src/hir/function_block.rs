use crate::{hir::variable::Variable, to_proto::{IterToProto, SymbolInfo}};

#[salsa::tracked(debug)]
pub struct FunctionBlock<'db> {
    pub input_variables: Vec<Variable<'db>>,
    pub output_variables: Vec<Variable<'db>>,
    pub in_out_variables: Vec<Variable<'db>>,
    pub temp_variables: Vec<Variable<'db>>,
    pub external_variables: Vec<Variable<'db>>,
    pub global_variables: Vec<Variable<'db>>,
    pub retain_variables: Vec<Variable<'db>>,
    pub no_retain_variables: Vec<Variable<'db>>,
    pub loc_partly_variables: Vec<Variable<'db>>,
}

