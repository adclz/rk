+++
title = "Linter"
description = "The lint rules rk applies, what each one catches, and how to configure or silence it."

[extra]
lede = "The lint rules rk applies, what each one catches, and how to configure or silence it."
md = "/linter/index.md"
+++
Lints are reported by `rk check` and by the language server in your editor.

They never run during `rk compile` or `rk test`, and they never change the exit code.

> [!NOTE]
> A lint cannot block a build or fail a pipeline on its own.

## Configuration

The linter is on by default with the **recommended** set: the rules that report a probable bug rather than a matter of style.

A workspace that never mentions the linter still gets them.
The `[linter]` section of `config.toml` tunes that set, it does not switch the linter on.

| `select` | Rules that run |
| --- | --- |
| absent | the recommended set |
| `"recommended"` | the same, written explicitly |
| `"all"` | every rule below |
| `"none"` | none, unless `[linter.rules]` names one |

```toml
[linter]
select = "all"           # the default is "recommended"

[linter.rules]
yoda-condition = false   # opt out of one that select turned on
```

`[linter.rules]` overrides `select` in both directions, so you can adopt the style rules one at a time instead of all at once.

- An unknown `select` value is a configuration error, and the message names the three valid ones.
- An unknown rule name in `[linter.rules]` is accepted and does nothing.

## Silencing one place

You can silence a rule at one specific place instead of turning it off everywhere, with the `{allow 'rule-name'}` pragma:

- Above a POU, it covers the whole POU.
- As a statement, it covers the next statement and everything nested in it.
- One pragma can take several rule names.

> [!IMPORTANT]
> A rule name that does not exist is reported as `L0005` and silences nothing.

```iecst sketch
{allow 'input-assignment'}
FUNCTION_BLOCK Rebinder
	…
END_FUNCTION_BLOCK

	{allow 'missing-input-param'}
	mb(REQ := TRUE, MODE := USINT#1);
```

{{ linter_table() }}
