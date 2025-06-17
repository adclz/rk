use std::ops::Deref;

use crate::hir;
use crate::hir::variable::{Array, BitString, Literal, PrimitiveKind, Variable, VariableKind};
use crate::parser::Parse;
use crate::ident::Ident;
use ast::generated::FuncVariables;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{BaseDatabase, File};

impl<'db> Parse<'db> for ast::generated::FuncDecl {
    type Output = hir::function::Function<'db>;

    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> Self::Output {
        let input_variables = self.parse_variables(db, file);
        hir::function::Function::new(db, input_variables, vec![], vec![], vec![], vec![], vec![])
    }
}

trait ParseVariable<'db> {
    fn parse_variables(&self, db: &'db dyn BaseDatabase, file: File) -> Vec<Variable<'db>>;
}

trait ToVariable<'db> {
    fn to_variables(&self, db: &'db dyn BaseDatabase, file: File) -> Variable<'db>;
}

trait ParseConstant<'db> {
    fn p(&self, db: &'db dyn BaseDatabase, file: File);
}

impl<'db> ParseConstant<'db> for ast::generated::ConstantExpr {

    fn p(&self, db: &'db dyn BaseDatabase, file: File) {
        type Constant = ast::generated::BitStrLiteral_BoolLiteral_CharLiteral_NumericLiteral_TimeLiteral;

        match self.children.children.deref() {
            Constant::BoolLiteral(bool_literal) => {
                Literal::Bool(Ident::from_node(db, file, bool_literal.value.deref()).unwrap());
            },
            Constant::BitStrLiteral(bit_str_literal) => {
                match bit_str_literal.int.deref() {
                    ast::generated::BinaryInt_HexInt_OctalInt_UnsignedInt::BinaryInt(binary_int) => {
                        let bits = binary_int.children
                            .iter()
                            .map(|bit| Ident::from_node(db, file, bit.deref()).unwrap())
                            .collect::<Vec<_>>();
                        Literal::BitString(BitString::Binary(Ident::join(db, &bits)))
                    },
                    ast::generated::BinaryInt_HexInt_OctalInt_UnsignedInt::HexInt(hex_int) => {
                        let hex = hex_int.children
                            .iter()
                            .map(|hex| Ident::from_node(db, file, hex.deref()).unwrap())
                            .collect::<Vec<_>>();
                        Literal::BitString(BitString::Hex(Ident::join(db, &hex)))
                    },
                    ast::generated::BinaryInt_HexInt_OctalInt_UnsignedInt::OctalInt(octal_int) => {
                        let octal = octal_int.children
                            .iter()
                            .map(|octal| Ident::from_node(db, file, octal.deref()).unwrap())
                            .collect::<Vec<_>>();
                        Literal::BitString(BitString::Octal(Ident::join(db, &octal)))
                    },
                    ast::generated::BinaryInt_HexInt_OctalInt_UnsignedInt::UnsignedInt(unsigned_int) => {
                        let unsigned = unsigned_int.children
                            .iter()
                            .map(|unsigned| Ident::from_node(db, file, unsigned.deref()).unwrap())
                            .collect::<Vec<_>>();
                        Literal::BitString(BitString::Unsigned(Ident::join(db, &unsigned)))
                    },
                };

            },
            Constant::CharLiteral(char_literal) => {
                match char_literal.char.children.deref() {
                    ast::generated::DByteCharStr_SByteCharStr::SByteCharStr(s_byte_char_str) => {
                        let char = Ident::from_node(db, file, s_byte_char_str.char.deref());
                        //Literal::Char(Ident::join(db, &char));
                    },
                    ast::generated::DByteCharStr_SByteCharStr::DByteCharStr(d_byte_char_str) => {
                        let char = Ident::from_node(db, file, d_byte_char_str.char.deref());
                        //Literal::Char(Ident::join(db, &char));
                    },
                }
            },
            Constant::NumericLiteral(numeric_literal) => {
                
            },
            Constant::TimeLiteral(time_literal) => todo!(),
        }
    }
}

impl<'db> ToVariable<'db> for ast::generated::VarDeclInit {
    fn to_variables(&self, db: &'db dyn BaseDatabase, file: File) -> Variable<'db> {
        match self.init.deref() {
            ast::generated::VarDeclKind::ArraySpecInit(array) => todo!(),
            ast::generated::VarDeclKind::StrVarDecl(_) => todo!(),
            ast::generated::VarDeclKind::SimpleVarDecl(_) => todo!(),
        }
    }
}

impl<'db> ParseVariable<'db> for ast::generated::FuncDecl {
    fn parse_variables(&self, db: &'db dyn BaseDatabase, file: File) -> Vec<Variable<'db>> {
        let mut intput_variables = vec![];
        self.variables
            .iter()
            .for_each(|variable| match variable.deref() {
                FuncVariables::InputDecls(variable) => {
                    variable
                        .children
                        .iter()
                        .for_each(|variable| match variable.deref() {
                            ast::generated::InputVarKind::ArrayConformDecl(array_conform_decl) => {
                                array_conform_decl.variables.children.iter().for_each(|variable| {
                                    intput_variables.push(Variable::from(
                                        db,
                                        variable.get_text(file.document(db).as_bytes()).unwrap(),
                                        VariableKind::Array(todo!()),
                                    ));
                                });
                            }
                            ast::generated::InputVarKind::EdgeDecl(edge_decl) => {
                                edge_decl.variables.children.iter().for_each(|variable| {
                                    intput_variables.push(Variable::from(
                                        db,
                                        variable.get_text(file.document(db).as_bytes()).unwrap(),
                                        VariableKind::Edge
                                    ));
                                });
                            }
                            ast::generated::InputVarKind::VarDeclInit(var_decl_init) => {
                                let kind = match var_decl_init.init.deref() {
                                    ast::generated::VarDeclKind::ArraySpecInit(_) => VariableKind::Array(todo!()),
                                    ast::generated::VarDeclKind::StrVarDecl(_) => VariableKind::Primitive(PrimitiveKind::String),
                                    ast::generated::VarDeclKind::SimpleVarDecl(_) => {
                                        todo!()
                                    }
                                };
                                
                                
                                var_decl_init
                                    .variables
                                    .children
                                    .iter()
                                    .for_each(|variable| {
                                        intput_variables.push(Variable::from(
                                            db,
                                            variable.get_text(file.document(db).as_bytes()).unwrap(),
                                            VariableKind::Primitive(PrimitiveKind::Bool),
                                        ));
                                    });
                            }
                        });
                }
                FuncVariables::OutputDecls(variable) => {}
                FuncVariables::InOutDecls(variable) => {}
                FuncVariables::ExternalVarDecls(variable) => {}
                FuncVariables::TempVarDecls(variable) => {}
                FuncVariables::VarDecls(variable) => {}
            });

        intput_variables
    }
}
