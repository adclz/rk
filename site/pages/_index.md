+++
title = "rk"
description = "A compiler and toolchain for IEC 61131-3 Structured Text, documented as Agent Skills the compiler verifies."

[extra]
lede = "**rk** compiles IEC 61131-3 Structured Text to WebAssembly: check, test, compile, in one binary, from any editor."
md = "/index.md"
+++
## Install the skills

{{ install() }}

Unpack it wherever your agent keeps its skills; any agent that reads the Agent Skills format can use them. Every skill is also a plain file at `/skills/<name>/SKILL.md`, if you want one on its own. New here? [getting-started](/skills/getting-started/) is the first one to read.

{{ skills() }}

## Diagnostics

{{ diagnostics_count() }} codes, one page per [category](/diagnostics/), each entry with the example that produces it and the compiler's own output. <br>The same data as [JSON](/diagnostics.json), which `rk explain` embeds.

## For agents too

Every page here is also Markdown, and this documentation is also a set of tools an agent can call.
