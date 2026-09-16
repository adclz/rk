+++
title = "Linter"
description = "The lint rules rk applies, what each one catches, and how to configure or silence it."

[extra]
lede = "Rules that read the same tree the compiler does, so a lint knows what a name means rather than how it is spelled."
md = "/linter/index.md"
eyebrow = "tools"
+++
Lints appear in `rk check` and in your editor. They never run during `rk compile` or `rk test`, and they never change an exit code, so a lint cannot block a build or fail a pipeline on its own.

## Configuration

The linter is on by default with the *recommended* set: the rules that report a probable bug rather than a preference. A workspace that never mentions the linter still gets them. `[linter]` tunes that set, it does not switch the linter on.

| `select` | Rules that run |
| --- | --- |
| absent | the recommended set |
| `"recommended"` | the same, said out loud |
| `"all"` | every rule below |
| `"none"` | none, unless `[linter.rules]` names one |

```toml
[linter]
select = "all"           # the default is "recommended"

[linter.rules]
yoda-condition = false   # opt out of one that select turned on
```

`[linter.rules]` overrides `select` both ways, so a style rule can be adopted one at a time rather than all at once. An unknown `select` value is a configuration error naming the three that exist; an unknown rule *name* is accepted and does nothing.

## Silencing one place

`{allow 'rule-name'}` silences a rule exactly where the code is deliberate, instead of turning it off everywhere. Above a POU it covers that POU; as a statement it covers the next statement and everything nested in it. One pragma takes several names. A name that does not exist is reported as `L0005` and silences nothing, because a typo must not silence the typo.

```iecst sketch
{allow 'input-assignment'}
FUNCTION_BLOCK Rebinder
	…
END_FUNCTION_BLOCK

	{allow 'missing-input-param'}
	mb(REQ := TRUE, MODE := USINT#1);
```

{{ linter_table() }}
