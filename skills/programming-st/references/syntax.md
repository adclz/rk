## Expressions and precedence

Tightest first: `^` deref; unary `-` `+` `NOT`; `**`; `*` `/` `MOD`; `+` `-`; `<` `>` `<=` `>=` `=` `<>`; `&` and `AND`; `XOR`; `OR`.

Two consequences worth memorising:

`AND`, `XOR` and `OR` bind **looser** than comparison, so a masked bit test needs parentheses. Without them the comparison eats the mask and you get E0301 (expected BOOL, got WORD):

```iecst fragment
ok := (w AND WORD#16#0008) <> WORD#0;     // correct
ok := w AND WORD#16#0008 <> WORD#0;       // E0301
```

`=` and `<>` sit at the same level as `<` `>` `<=` `>=`, not below them.

`**` requires a `REAL` or `LREAL` base (E0318 on an integer base) and is left-associative: `2.0 ** 3.0 ** 2.0` is 64, not 512.

`NOT`, `AND`, `OR`, `XOR` are bitwise on `BYTE`/`WORD`/`DWORD`/`LWORD` and logical on `BOOL`.
Integer division truncates. Integer arithmetic wraps at the declared IEC width — `INT#32767 + 1` is `-32768`.

## Statements

```iecst sketch
x := expr;

IF a THEN … ELSIF b THEN … ELSE … END_IF;

CASE selector OF
	0: …
	1, 2: …           // list
	3..7: …           // range
	Color#Red: …      // enum labels, qualified
ELSE
	…
END_CASE;

FOR i := 0 TO 10 BY 2 DO … END_FOR;      // BY optional, defaults to 1
WHILE cond DO … END_WHILE;
REPEAT … UNTIL cond END_REPEAT;

EXIT;        // leave the innermost loop
CONTINUE;    // next iteration
RETURN;      // no operand

__RAISE('message');   // throws to the host; there is no in-language catch
```

`EXIT` outside a loop is E1002 and `CONTINUE` outside a loop is E1001. A `FOR` step must fold to a non-zero compile-time constant (E1007); a `VAR CONSTANT` or a constant expression is fine, a plain variable is not. After a normal `FOR` completion the control variable holds the first value past the bound, EXCEPT when the bound is the control type's maximum, where it wraps instead: do not use it to detect completion.

Comments: `// to end of line`, `/* … */`, `(* … *)`.

## Calling

```iecst fragment
VAR
	c: Counter;                      // a FUNCTION_BLOCK needs an instance
END_VAR
n := Sum(1, 2);                      // positional
n := Sum(a := 1, b := 2);            // named
n := Sum(1, b := 2);                 // mixed is accepted
n := Sum(a := 1, b := 2, carry => ov);   // bind a VAR_OUTPUT with =>

c(CU := trig, PV := 5);              // call the instance
c(CU := trig, Q => done, CV => n);   // read outputs inline…
done := c.Q;                         // …or off the instance afterwards
c.Reset();                           // methods
```

Calling the FB *type* rather than an instance is E0229. Inside a `FUNCTION_BLOCK` or `CLASS` body, `THIS^.Method()`, `SUPER^.Method()` and `SUPER()` are available.

`FUNCTION` overloading is supported: several POUs may share a name as long as their signatures differ, and the call site picks by argument types. Two POUs with the *same* signature are E0101.

## Namespaces

```iecst
NAMESPACE App.Motors                 // dotted names allowed
	USING Std.Convert;               // directives come first

	FUNCTION Clamp: INT
		VAR_INPUT
			v: INT;
		END_VAR
		Clamp := v;
	END_FUNCTION

	TYPE Level : INT; END_TYPE

	NAMESPACE INTERNAL Detail
		FUNCTION Helper: INT
			Helper := 0;
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE

USING App.Motors;                    // at file level, or at the top of a POU header

FUNCTION UseIt: INT
	VAR
		s: App.Motors.Level;         // fully qualified always works
	END_VAR
	UseIt := Clamp(v := 1);          // unqualified, thanks to USING
END_FUNCTION
```

`PROGRAM` and `CONFIGURATION` may not appear inside a `NAMESPACE` (E0022 /
E0023). The `programming-namespaces` skill covers fragments, resolution,
ambiguity and the visibility specifiers.
