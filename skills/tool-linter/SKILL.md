---
name: tool-linter
description: The lint rules `rk` applies, what each L-code means and how to configure or silence one. Use when a lint fires and its intent is unclear, or when tuning the linter in config.toml.
---

## Summary

The linter runs on top of the compiler diagnostics and reports style, clarity and suspicious-code findings. It is ON by default: a workspace that never mentions the linter still gets the **recommended** set — the 21 rules that report a probable bug rather than a matter of taste, which are exactly the ones that emit at warning severity. `[linter]` tunes that set; it does not switch the linter on.

Only `rk check` runs the linter. `rk compile` and `rk test` skip it entirely, so a lint can never block a build. Lints are also never reported for library or standard-library files, only for the workspace's own sources. No lint carries a quick fix, so no code action is offered for one.

Lints never change the exit code. `rk check` exits non-zero on errors only; a workspace full of warnings still exits 0.

Each lint diagnostic carries a note naming the rule that produced it, which is the name used to disable it:

```sh
rk check
[L0207] Hint: empty CASE branch
   ...
   │ Note: lint rule: empty-case-branch
```

The `json-lines` format carries the same string in the `notes` array. The `concise` format drops it, so use `full` or `json-lines` when you need the rule name.

`rk explain` accepts lint codes as well as error codes: `rk explain L0101` prints the title, category and a description of the rule.

## Configuration

Two layers. `select` picks the baseline; `[linter.rules]` overrides individual
rules on top of it, in both directions.

| `select` | rules that run |
| -------- | -------------- |
| absent, or no `[linter]` at all | the 21 recommended (warning-severity) rules |
| `"recommended"` | the same 20, stated explicitly |
| `"all"` | all 49 |
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

To run the style rules on top of the default instead of taking all 49:

```toml
[linter]
[linter.rules]
yoda-condition = true    # opt IN one the baseline leaves out
```

An unknown value for `select` is a config error naming the three valid ones.
Unknown rule NAMES in `[linter.rules]` are still accepted silently and do
nothing — copy the names from the tables below.

To silence one deliberate site instead of a whole rule, `{allow 'rule-name' ...}`:
above a POU (or METHOD) it covers that whole POU; as a statement it covers the
NEXT statement, nested bodies included. Several names fit one pragma. An unknown
name there is NOT silent: `unknown-allow` (L0005) reports it, and it silences
nothing.

Severity is what decides the baseline: every warning-severity rule is
recommended, every info and hint rule is opt-in. The tables below carry the
severity, so they double as the list of what runs by default.

## Pragmas (L00xx)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0001 | `warn-pragma` | info | the message of an `{info = '...'}` pragma, at each call site of the marked POU |
| L0002 | `warn-pragma` | warning | the message of a `{warn = '...'}` pragma, at each call site of the marked POU |
| L0003 | `invalid-pragma` | warning | a pragma placed on a POU kind that does not accept it: `{test}` on FUNCTION_BLOCK, METHOD or PROGRAM, `{once}` on PROGRAM |
| L0004 | `once-violation` | info | a `{once}` POU called more than once in the same body |
| L0005 | `unknown-allow` | warning | an `{allow}` pragma naming a rule that does not exist |

L0001 and L0002 share the rule name `warn-pragma`; disabling it silences both.

## Declarations and naming (L01xx)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0101 | `unused-variable` | info | a VAR, VAR_INPUT or VAR_TEMP declaration never used in the body |
| L0102 | `shadowing-variable` | info | a variable with the same name as a POU visible in the scope |
| L0103 | `duplicate-var-section` | info | the same variable section opened twice in one POU |
| L0104 | `negated-condition` | info | `IF NOT c THEN ... ELSE ...`, which reads better with the branches swapped |
| L0106 | `unnecessary-else` | info | an ELSE branch whose preceding branches all end with RETURN, EXIT or CONTINUE |
| L0107 | `bool-comparison` | info | `x = TRUE`, `x <> FALSE` and the other comparisons against a boolean literal |
| L0108 | `redundant-not` | info | double negation `NOT NOT x` |
| L0109 | `duplicate-namespace` | info | the same NAMESPACE reopened in the same file |
| L0110 | `single-element-array` | info | an array dimension whose lower and upper bounds are equal |
| L0111 | `negated-comparison` | info | `NOT (x = y)`, which is `x <> y` |
| L0112 | `duplicate-configuration` | info | two same-named CONFIGURATION blocks in the same file |

`unused-variable` never reports VAR_OUTPUT, VAR_IN_OUT, VAR_GLOBAL, VAR_EXTERNAL, VAR_CONFIG or VAR_ACCESS, since those are read or written from outside the POU. It also skips the VAR_INPUT of a PROGRAM (written by the CONFIGURATION), the whole body of an `{extern}` FUNCTION, and any name starting with an underscore, which is the way to mark a declaration as deliberately unused.

## Style and clarity (L02xx)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0201 | `unused-import` | hint | a USING directive that nothing in the file resolves through |
| L0202 | `unused-return-type` | hint | a call whose return value is discarded |
| L0203 | `case-without-else` | hint | a CASE statement with no ELSE branch |
| L0204 | `missing-input-param` | hint | a FUNCTION_BLOCK or PROGRAM call that does not pass every declared VAR_INPUT |
| L0205 | `uninitialized-output` | info | VAR_OUTPUT declarations with no initializer that the body never assigns |
| L0206 | `empty-body` | hint | a FUNCTION, FUNCTION_BLOCK, METHOD or PROGRAM with no statements |
| L0207 | `empty-case-branch` | hint | a CASE branch with no statements |
| L0208 | `unnecessary-parens` | hint | parentheses around a bare literal, variable or enum value |
| L0209 | `yoda-condition` | hint | a literal on the left-hand side of a comparison |
| L0210 | `collapsible-if` | hint | a nested IF with no ELSE, which collapses into `IF a AND b THEN` |
| L0211 | `empty-if-branch` | hint | an IF, ELSIF or ELSE branch with no statements |
| L0212 | `effectless-statement` | hint | a bare expression used as a statement, such as `x;` |
| L0213 | `empty-loop-body` | hint | a FOR, WHILE or REPEAT loop with no statements |
| L0214 | `empty-type` | hint | a STRUCT with no fields or an ENUM with no variants |
| L0215 | `default-for-step` | hint | an explicit `BY 1`, which is already the default |

The rule name for L0202 is `unused-return-type`, not `unused-return-value`, even though the message reads "unused return value".

L0204 covers FUNCTION_BLOCK and PROGRAM call sites only. An incomplete FUNCTION or METHOD call is a hard error, E0233, and is not affected by this rule.

L0205 collapses every unassigned output of one body into a single diagnostic listing the names, with a related span per declaration.

`empty-body` never reports an `{extern}` FUNCTION, whose body is empty by definition.

## Suspicious code (L03xx)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0301 | `dead-code` | warning | a statement following RETURN, `__RAISE`, EXIT or CONTINUE in the same block |
| L0302 | `for-loop-step-sign` | warning | a FOR step whose direction contradicts the bounds, such as `FOR i := 10 TO 1 BY 1` |
| L0303 | `input-assignment` | warning | an assignment to a VAR_INPUT |
| L0304 | `constant-condition` | warning | an IF, ELSIF, WHILE or UNTIL condition written as the literal TRUE or FALSE |
| L0305 | `division-by-zero` | warning | a literal `0` on the right-hand side of `/` or `MOD` |
| L0306 | `duplicate-case` | warning | a CASE selector or range already covered by an earlier branch, including overlapping ranges |
| L0308 | `loop-var-modified` | warning | an assignment to a FOR control variable inside the loop body |
| L0309 | `self-assignment` | warning | `x := x` |
| L0310 | `self-comparison` | warning | `x = x`, `x <> x`, `x > x` and the rest, whose result is constant; not on a REAL or LREAL, where `x <> x` is the NaN test |
| L0311 | `identical-sub-expr` | warning | `a AND a`, `a OR a`, `a XOR a` |
| L0312 | `identity-operation` | warning | `* 1`, `1 *`, `/ 1`, `+ 0`, `0 +`, `- 0` |
| L0313 | `sub-self` | warning | `x - x` on an integer, always 0; on a float it is the finiteness test and is not reported |
| L0314 | `constant-loop-bounds` | warning | a FOR loop whose start and end are the same value, so the body runs exactly once |
| L0315 | `self-shadowing` | warning | a variable with the same name as the POU or method it is declared in |
| L0316 | `missing-return` | warning | a FUNCTION or METHOD with a return type that never assigns the return value; a `{wasm}` statement whose `(result)` is the FUNCTION counts |
| L0317 | `external-mutation` | warning | writing a field of a function block or class instance from outside it, `inst.x := 42` |
| L0318 | `method-shadows-member` | warning | a method local or parameter with the same name as a member of its FUNCTION_BLOCK or CLASS |

L0306 keys on the value the compiler computed, not on the text, so `7`, `INT#7` and a CONSTANT holding 7 are one label; enum variants and strings fall back to the written form.

L0304 only looks at a literal TRUE or FALSE. A condition that is constant after folding is not reported.

## Globals (L04xx)

| Code | Rule | Severity | Flags |
| --- | --- | --- | --- |
| L0410 | `global-without-external` | warning | reading or writing a CONFIGURATION VAR_GLOBAL by bare name, with no matching VAR_EXTERNAL in the POU |

The code is accepted and compiles; strict IEC 61131-3 wants the global imported through VAR_EXTERNAL first.

## Codes with no rule

L0105 and L0307 are not assigned. `rk explain` rejects them.
