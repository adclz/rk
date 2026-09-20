---
name: tool-linter
description: The lint rules `rk` applies, what each L-code means and how to configure or silence one. Use when a lint fires and its intent is unclear, or when tuning the linter in config.toml.
---

## Summary

The linter runs on top of the compiler diagnostics and reports style, clarity and suspicious-code findings.
It is ON by default: a workspace that never mentions the linter still gets the **recommended** set — the 22 rules that report a probable bug rather than a matter of taste.
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
| absent, or no `[linter]` at all | the 22 recommended rules |
| `"recommended"` | the same 22, stated explicitly |
| `"all"` | all 48 |
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

To run the style rules on top of the default instead of taking all 48:

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

## Declarations and naming (L01xx)

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

## Style and clarity (L02xx)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0206 | `uninitialized-output` | info | VAR_OUTPUT declarations with no initializer that the body never assigns |
| L0213 | `default-for-step` | hint | an explicit `BY 1`, which is already the default |
| L0301 | `unused-import` | hint | a USING directive that nothing in the file resolves through |
| L0302 | `unused-return-type` | hint | a call whose return value is discarded |
| L0303 | `missing-input-param` | hint | a FUNCTION_BLOCK or PROGRAM call that does not pass every declared VAR_INPUT |
| L0304 | `case-without-else` | hint | a CASE statement with no ELSE branch |
| L0305 | `empty-body` | hint | a FUNCTION, FUNCTION_BLOCK, METHOD or PROGRAM with no statements |
| L0306 | `empty-if-branch` | hint | an IF, ELSIF or ELSE branch with no statements |
| L0307 | `empty-case-branch` | hint | a CASE branch with no statements |
| L0308 | `empty-loop-body` | hint | a FOR, WHILE or REPEAT loop with no statements |
| L0309 | `empty-type` | hint | a STRUCT with no fields or an ENUM with no variants |
| L0310 | `effectless-statement` | hint | a bare expression used as a statement, such as `x;` |
| L0311 | `unnecessary-parens` | hint | parentheses around a bare literal, variable or enum value |
| L0312 | `collapsible-if` | hint | a nested IF with no ELSE, which collapses into `IF a AND b THEN` |
| L0313 | `yoda-condition` | hint | a literal on the left-hand side of a comparison |

The rule name for L0302 is `unused-return-type`, not `unused-return-value`, even though the message reads "unused return value".

L0303 covers FUNCTION_BLOCK and PROGRAM call sites only.
An incomplete FUNCTION or METHOD call is a hard error, E0802, and is not affected by this rule.

L0206 collapses every unassigned output of one body into a single diagnostic listing the names, with a related span per declaration.

`empty-body` never reports an `{extern}` FUNCTION, whose body is empty by definition.

## Suspicious code (L03xx)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0101 | `dead-code` | warning | a statement following RETURN, `__RAISE`, EXIT or CONTINUE in the same block |
| L0102 | `division-by-zero` | warning | a literal `0` on the right-hand side of `/` or `MOD` |
| L0103 | `constant-condition` | warning | an IF, ELSIF, WHILE or UNTIL condition written as the literal TRUE or FALSE |
| L0104 | `self-assignment` | warning | `x := x` |
| L0105 | `self-comparison` | warning | `x = x`, `x <> x`, `x > x` and the rest, whose result is constant; not on a REAL or LREAL, where `x <> x` is the NaN test |
| L0106 | `sub-self` | warning | `x - x` on an integer, always 0; on a float it is the finiteness test and is not reported |
| L0107 | `identity-operation` | warning | `* 1`, `1 *`, `/ 1`, `+ 0`, `0 +`, `- 0` |
| L0108 | `identical-sub-expr` | warning | `a AND a`, `a OR a`, `a XOR a` |
| L0109 | `duplicate-case` | warning | a CASE selector or range already covered by an earlier branch, including overlapping ranges |
| L0110 | `for-loop-step-sign` | warning | a FOR step whose direction contradicts the bounds, such as `FOR i := 10 TO 1 BY 1` |
| L0111 | `constant-loop-bounds` | warning | a FOR loop whose start and end are the same value, so the body runs exactly once |
| L0112 | `loop-var-modified` | warning | an assignment to a FOR control variable inside the loop body |
| L0113 | `input-assignment` | warning | an assignment to a VAR_INPUT |
| L0114 | `missing-return` | warning | a FUNCTION or METHOD with a return type that never assigns the return value; a `{wasm}` statement whose `(result)` is the FUNCTION counts |
| L0115 | `self-shadowing` | warning | a variable with the same name as the POU or method it is declared in |
| L0116 | `method-shadows-member` | warning | a method local or parameter with the same name as a member of its FUNCTION_BLOCK or CLASS |
| L0117 | `external-mutation` | warning | writing a field of a function block or class instance from outside it, `inst.x := 42` |

L0109 keys on the value the compiler computed, not on the text, so `7`, `INT#7` and a CONSTANT holding 7 are one label; enum variants and strings fall back to the written form.

L0103 only looks at a literal TRUE or FALSE.
A condition that is constant after folding is not reported.

## Globals (L04xx)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0118 | `global-without-external` | warning | reading or writing a CONFIGURATION VAR_GLOBAL by bare name, with no matching VAR_EXTERNAL in the POU |

The code is accepted and compiles; strict IEC 61131-3 wants the global imported through VAR_EXTERNAL first.

