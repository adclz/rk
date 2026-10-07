---
name: tool-linter
description: The lint rules `rk` applies, what each L-code means and how to configure or silence one. Use when a lint fires and its intent is unclear, or when tuning the linter in config.toml.
---

## Summary

The linter runs on top of the compiler diagnostics and reports style, clarity and suspicious-code findings.
It is ON by default: a workspace that never mentions the linter still gets the **recommended** set — the 27 rules that report a probable bug rather than a matter of taste.
`[linter]` tunes that set; it does not switch the linter on.

`rk check` and the language server both run it, on the same set, so an editor and the CLI agree about a workspace.
`rk compile` and `rk test` skip it entirely, so a lint can never block a build.
Lints are also never reported for library or standard-library files, only for the workspace's own sources.
No lint carries a quick fix, so no code action is offered for one.

Lints never change the exit code.
`rk check` exits non-zero on errors only; a workspace full of warnings still exits 0.

Each lint diagnostic carries a note naming the rule that produced it, which is the name used to disable it:

```sh
rk check
[L0307] Hint: empty CASE branch
   ...
   │ Note: lint rule: empty-case-branch
```

The `json-lines` format carries the same string in the `notes` array.
The `concise` format drops it, so use `full` or `json-lines` when you need the rule name.

`rk explain` accepts lint codes as well as error codes: `rk explain L0201` prints the title, category and a description of the rule.

## Configuration

Two layers.
`select` picks the baseline; `[linter.rules]` overrides individual rules on top of it, in both directions.

| `select` | rules that run |
| -------- | -------------- |
| absent, or no `[linter]` at all | the 27 recommended rules |
| `"recommended"` | the same 27, stated explicitly |
| `"all"` | all 54 |
| `"none"` | none, unless `[linter.rules]` names one |

```toml
[project]
name = "myproject"
version = "0.1.0"

[linter]
select = "all"           # default is "recommended"

[linter.rules]
yoda-condition = false   # opt OUT of one `select` turned on
```

To run the style rules on top of the default instead of taking all 54:

```toml
[linter]
[linter.rules]
yoda-condition = true    # opt IN one the baseline leaves out
```

An unknown value for `select` is a config error naming the three valid ones.
Unknown rule NAMES in `[linter.rules]` are still accepted silently and do nothing — copy the names from the tables below.

To silence one deliberate site instead of a whole rule, `{allow 'rule-name' ...}`: above a POU (or METHOD) it covers that whole POU; as a statement it covers the NEXT statement, nested bodies included.
Several names fit one pragma.
An unknown name there is NOT silent: `unknown-allow` (L0005) reports it, and it silences nothing.

Severity almost decides the baseline: every warning-severity rule is recommended, and info and hint rules are opt-in with two exceptions, marked **(recommended)** in the tables below.
L0001 and L0202 report at info severity and still run by default, because what they flag is a probable bug that is quiet enough not to warrant a warning.

## Pragmas (L00xx)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0001 | `warn-pragma` | info **(recommended)** | the message of an `{info = '...'}` pragma, at each call site of the marked POU |
| L0002 | `warn-pragma` | warning | the message of a `{warn = '...'}` pragma, at each call site of the marked POU |
| L0003 | `invalid-pragma` | warning | a pragma placed on a POU kind that does not accept it: `{test}` on FUNCTION_BLOCK, METHOD or PROGRAM, `{once}` on PROGRAM |
| L0004 | `once-violation` | info | a `{once}` POU called more than once in the same body |
| L0005 | `unknown-allow` | warning | an `{allow}` pragma naming a rule that does not exist |

L0001 and L0002 share the rule name `warn-pragma`; disabling it silences both.

## Declarations and naming

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0201 | `unused-variable` | info | a VAR, VAR_INPUT or VAR_TEMP declaration never used in the body |
| L0202 | `shadowing-variable` | info **(recommended)** | a variable with the same name as a POU visible in the scope |
| L0203 | `duplicate-var-section` | info | the same variable section opened twice in one POU |
| L0204 | `duplicate-namespace` | info | the same NAMESPACE reopened in the same file |
| L0205 | `duplicate-configuration` | info | two same-named CONFIGURATION blocks in the same file |
| L0207 | `negated-condition` | info | `IF NOT c THEN ... ELSE ...`, which reads better with the branches swapped |
| L0208 | `negated-comparison` | info | `NOT (x = y)`, which is `x <> y` |
| L0209 | `bool-comparison` | info | `x = TRUE`, `x <> FALSE` and the other comparisons against a boolean literal |
| L0210 | `redundant-not` | info | double negation `NOT NOT x` |
| L0211 | `unnecessary-else` | info | an ELSE branch whose preceding branches all end with RETURN, EXIT or CONTINUE |
| L0212 | `single-element-array` | info | an array dimension whose lower and upper bounds are equal |

`unused-variable` never reports VAR_OUTPUT, VAR_IN_OUT, VAR_GLOBAL, VAR_EXTERNAL, VAR_CONFIG or VAR_ACCESS, since those are read or written from outside the POU.
It also skips the VAR_INPUT of a PROGRAM (written by the CONFIGURATION), the whole body of an `{extern}` FUNCTION, and any name starting with an underscore, which is the way to mark a declaration as deliberately unused.

## Style and clarity

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0206 | `uninitialized-output` | info | VAR_OUTPUT declarations with no initializer that the body never assigns |
| L0213 | `default-for-step` | hint | an explicit `BY 1`, which is already the default |
| L0214 | `aggregate-copy` | info | an assignment or a call binding that copies an ARRAY, a STRUCT or a FUNCTION_BLOCK or CLASS instance whole, which costs as much as the type is large; a STRING copies its text only and is not reported |
| L0301 | `unused-import` | hint | a USING directive that nothing in the file resolves through |
| L0302 | `unused-return-type` | hint | a call whose return value is discarded |
| L0303 | `missing-input-param` | hint | a FUNCTION_BLOCK or PROGRAM call that does not pass every declared VAR_INPUT; an input the body writes through the same instance, `inst.x := 1`, counts as passed |
| L0304 | `case-without-else` | hint | a CASE statement with no ELSE branch, unless its labels take every variant of the selector's enum or its whole integer or subrange range; on an enum it names the variants left out |
| L0305 | `empty-body` | hint | a FUNCTION, FUNCTION_BLOCK, METHOD or PROGRAM with no statements |
| L0306 | `empty-if-branch` | hint | an IF, ELSIF or ELSE branch with no statements |
| L0307 | `empty-case-branch` | hint | a CASE branch with no statements |
| L0308 | `empty-loop-body` | hint | a FOR, WHILE or REPEAT loop with no statements |
| L0309 | `empty-type` | hint | a STRUCT with no fields or an ENUM with no variants |
| L0310 | `effectless-statement` | hint | a bare expression used as a statement, such as `x;` |
| L0311 | `unnecessary-parens` | hint | parentheses around a bare literal, variable or enum value |
| L0312 | `collapsible-if` | hint | a nested IF with no ELSE, which collapses into `IF a AND b THEN` |
| L0313 | `yoda-condition` | hint | a literal on the left-hand side of a comparison |
| L0314 | `positional-output` | hint | a positional argument in the place of a VAR_OUTPUT, which receives the output |

The rule name for L0302 is `unused-return-type`, not `unused-return-value`, even though the message reads "unused return value".

L0303 covers FUNCTION_BLOCK and PROGRAM call sites only.
An incomplete FUNCTION or METHOD call is a hard error, E0802, and is not affected by this rule.

L0206 collapses every unassigned output of one body into a single diagnostic listing the names, with a related span per declaration.

`empty-body` never reports an `{extern}` FUNCTION, whose body is empty by definition.

## Suspicious code

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0101 | `dead-code` | warning | a statement following RETURN, `__RAISE`, EXIT or CONTINUE in the same block |
| L0102 | `division-by-zero` | warning | a literal `0` on the right-hand side of `/` or `MOD` |
| L0103 | `constant-condition` | warning | an IF, ELSIF, WHILE or UNTIL condition written as the literal TRUE or FALSE, and any comparison the type of its value decides, `u < 0` on an unsigned integer or `l > 10` on an `INT (0..10)` |
| L0104 | `self-assignment` | warning | `x := x` |
| L0105 | `self-comparison` | warning | `x = x`, `x <> x`, `x > x` and the rest, whose result is constant; not on a REAL or LREAL, where `x <> x` is the NaN test |
| L0106 | `sub-self` | warning | `x - x` on an integer, always 0; on a float it is the finiteness test and is not reported |
| L0107 | `identity-operation` | warning | `* 1`, `1 *`, `/ 1`, `+ 0`, `0 +`, `- 0` |
| L0108 | `identical-sub-expr` | warning | `a AND a`, `a OR a`, `a XOR a` |
| L0109 | `duplicate-case` | warning | a CASE selector or range already covered by an earlier branch, including overlapping ranges |
| L0110 | `for-loop-step-sign` | warning | a FOR step whose direction contradicts the bounds, such as `FOR i := 10 TO 1 BY 1` |
| L0111 | `constant-loop-bounds` | warning | a FOR loop whose start and end are the same value, so the body runs exactly once |
| L0112 | `loop-var-modified` | warning | an assignment to a FOR control variable inside the loop body |
| L0113 | `input-assignment` | warning | a POU assigning its own VAR_INPUT, `x := 1` or `THIS.x := 1`; setting another block's input, `t.x := 1`, is not reported |
| L0114 | `missing-return` | warning | a FUNCTION or METHOD with a return type that never assigns the return value; a `{wasm}` statement whose `(result)` is the FUNCTION counts |
| L0115 | `self-shadowing` | warning | a variable with the same name as the POU or method it is declared in |
| L0116 | `method-shadows-member` | warning | a method local or parameter with the same name as a member of its FUNCTION_BLOCK or CLASS |
| L0117 | `external-mutation` | warning | writing a field of a function block or class instance from outside it, `inst.x := 42`; an input is not reported, writing it before the call is how it is passed |
| L0119 | `instance-in-function` | warning | a FUNCTION or METHOD holding or returning a FUNCTION_BLOCK or CLASS instance, which starts over at every call: a timer in it never expires |
| L0120 | `variable-method-name` | warning | a variable and a method of a FUNCTION_BLOCK or CLASS, or of one it extends, with the same name: legal, the variable wins inside the block |
| L0121 | `latin1-escape` | warning | a STRING literal whose `$hh` escapes are not UTF-8 text, such as `'caf$E9'` written for `'café'`: `$E9` is one byte, and `é` is `$C3$A9`; not a CHAR, where `$E9` is `é` |
| L0122 | `recursion` | warning | a call that leads back to the POU making it, directly or through others; each call gets its own frame on a stack that holds the largest frame and 64 KiB more, or the `stack_size` set in `config.toml`, and too deep a recursion stops the program with `stack overflow` |
| L0123 | `negative-radix-literal` | warning | an untyped radix literal with its top bit set where a signed integer is expected: `16#80` is -128 in a SINT and 128 in an INT; not a typed `SINT#16#80`, nor one under a minus, `-(16#80)`, nor an operand of AND, OR or XOR |
| L0124 | `float-equality` | warning | two REAL or LREAL values compared with `=` or `<>`, which a rounding difference makes FALSE; not against a literal zero, nor `x <> x`, the NaN test |
| L0126 | `endless-loop` | warning | a WHILE or REPEAT whose body changes nothing its condition reads, with no EXIT, RETURN or `__RAISE`: waiting for an input in a loop, whose next value comes with the next scan; when the body calls something, only a FUNCTION's or METHOD's own locals count as unchanged |
| L0127 | `string-truncation` | warning | a STRING variable assigned, passed to an input or bound from an output into a STRING declared shorter, which cuts the text; not a literal (E0314), nor a call's result |
| L0128 | `constant-overflow` | warning | an operation on constants whose result the type it runs at cannot hold, which the program computes wrapped: `200 * 200` multiplies two INTs, so `N : DINT := 200 * 200` holds -25536; in a body, an initializer, a STRING length or an array, subrange or enum bound |

L0109 keys on the value the compiler computed, not on the text, so `7`, `INT#7` and a CONSTANT holding 7 are one label; enum variants and strings fall back to the written form.

L0103 reads a condition as written, or a comparison against the range of its value's type.
A comparison of two constants is not reported, nor a condition that is constant only after folding.

## Globals

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0118 | `global-without-external` | warning | reading or writing a CONFIGURATION VAR_GLOBAL by bare name, with no matching VAR_EXTERNAL in the POU |

The code is accepted and compiles; strict IEC 61131-3 wants the global imported through VAR_EXTERNAL first.

