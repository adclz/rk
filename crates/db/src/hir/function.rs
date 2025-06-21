
use crate::{hir::variable::Variable};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    pub input_variables: Vec<Variable<'db>>,
    pub output_variables: Vec<Variable<'db>>,
    pub in_out_variables: Vec<Variable<'db>>,
    pub temp_variables: Vec<Variable<'db>>,
    pub external_variables: Vec<Variable<'db>>,
    pub global_variables: Vec<Variable<'db>>,
}