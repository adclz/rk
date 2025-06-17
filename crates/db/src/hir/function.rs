
use crate::{hir::variable::Variable};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    input_variables: Vec<Variable<'db>>,
    output_variables: Vec<Variable<'db>>,
    in_out_variables: Vec<Variable<'db>>,
    temp_variables: Vec<Variable<'db>>,
    external_variables: Vec<Variable<'db>>,
    global_variables: Vec<Variable<'db>>,
}