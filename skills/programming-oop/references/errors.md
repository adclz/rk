## Error codes

`E0001` Multiple EXTENDS declarations.

`E0003` IMPLEMENTS declared before EXTENDS.

`E0024` VAR_IN_OUT not allowed in this context (a CLASS).

`E0025` VAR_TEMP not allowed in this context (a CLASS, an interface prototype).

`E0037` Access specifier on an interface method prototype.

`E0038` METHOD declared inside a body (a PROGRAM).

`E0229` Not a callable type; a CLASS instance cannot be invoked.

`E0401` Cannot access a PRIVATE item.

`E0402` Cannot access an INTERNAL item from another namespace.

`E0403` Cannot access a PROTECTED item outside the POU or its derived POUs.

`E0501` `SUPER()` not valid in this context.

`E0502` `SUPER` not valid in this context.

`E0503` `THIS` not valid in this context.

`E0504` Cannot override a FINAL method.

`E0505` Missing OVERRIDE when redefining a base method.

`E0506` Missing implementation of an ABSTRACT method by a CONCRETE derived POU.

`E0507` OVERRIDE on a method that exists in no base.

`E0510` ABSTRACT method declared in a POU that is not itself ABSTRACT.

`E0511` Instantiation of an ABSTRACT CLASS or FUNCTION_BLOCK.

`E0522` Extending a FINAL CLASS or FUNCTION_BLOCK.

`E0509` Unimplemented interface method.

`E0512` Method parameter count mismatch: the implementation declares a different number of parameters than the prototype or base method.
`E0523` Method parameter type mismatch: the parameter at a position has a different type; widening does not apply.
`E0524` Method return type mismatch: a different return type, or one where the base has none.
`E0525` Method parameter name mismatch: parameters are matched by position, and the name at each position is part of the signature.
`E0526` Method parameter section mismatch: `VAR_INPUT` in the prototype, `VAR_IN_OUT` or `VAR_OUTPUT` in the implementation; the section decides how the argument is passed.

`E0513` `SUPER` used with no EXTENDS clause.

`E0514` Interface type outside a FUNCTION / METHOD input or in-out parameter.

`E0515` Interface used as a return type.

`E0516` Interface nested in an array, a reference or a struct.

`E0517` Assignment to an interface parameter.

`E0518` `SUPER()` called in a method.

`E0519` `SUPER()` called more than once.

`E0520` `SUPER()` called inside a loop.

`E0521` Derived function block or class redeclares an inherited variable name.

Run `rk explain <code>` for the long form of any of these.
