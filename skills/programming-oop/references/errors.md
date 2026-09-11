## Error codes

`E1101` Multiple EXTENDS declarations.

`E1103` IMPLEMENTS declared before EXTENDS.

`E0018` VAR_IN_OUT not allowed in this context (a CLASS).

`E0019` VAR_TEMP not allowed in this context (a CLASS, an interface prototype).

`E1120` Access specifier on an interface method prototype.

`E0028` METHOD declared inside a body (a PROGRAM).

`E0808` Not a callable type; a CLASS instance cannot be invoked.

`E1001` Cannot access a PRIVATE item.

`E1003` Cannot access an INTERNAL item from another namespace.

`E1002` Cannot access a PROTECTED item outside the POU or its derived POUs.

`E1108` `SUPER()` not valid in this context.

`E1106` `SUPER` not valid in this context.

`E1105` `THIS` not valid in this context.

`E1114` Cannot override a FINAL method.

`E1112` Missing OVERRIDE when redefining a base method.

`E1116` Missing implementation of an ABSTRACT method by a CONCRETE derived POU.

`E1113` OVERRIDE on a method that exists in no base.

`E1117` ABSTRACT method declared in a POU that is not itself ABSTRACT.

`E1118` Instantiation of an ABSTRACT CLASS or FUNCTION_BLOCK.

`E1104` Extending a FINAL CLASS or FUNCTION_BLOCK.

`E1119` Unimplemented interface method.

`E1125` Method parameter count mismatch: the implementation declares a different number of parameters than the prototype or base method.
`E1126` Method parameter type mismatch: the parameter at a position has a different type; widening does not apply.
`E1127` Method return type mismatch: a different return type, or one where the base has none.
`E1128` Method parameter name mismatch: parameters are matched by position, and the name at each position is part of the signature.
`E1129` Method parameter section mismatch: `VAR_INPUT` in the prototype, `VAR_IN_OUT` or `VAR_OUTPUT` in the implementation; the section decides how the argument is passed.

`E1107` `SUPER` used with no EXTENDS clause.

`E1121` Interface type outside a FUNCTION / METHOD input or in-out parameter.

`E1122` Interface used as a return type.

`E1123` Interface nested in an array, a reference or a struct.

`E1124` Assignment to an interface parameter.

`E1109` `SUPER()` called in a method.

`E1110` `SUPER()` called more than once.

`E1111` `SUPER()` called inside a loop.

`E1115` Derived function block or class redeclares an inherited variable name.

Run `rk explain <code>` for the long form of any of these.
