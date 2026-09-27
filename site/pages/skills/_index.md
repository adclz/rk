+++
title = "Skills"
description = "The rk documentation as Agent Skills."
sort_by = "none"
# The page is the archive now: the menu downloads it. The skills stay
# pages of their own, and this list stays as Markdown for agents.
render = false

[extra]
lede = "Each skill is a folder an agent loads when a task matches its description. Together they are the language and toolchain documentation."
md = "/skills/index.md"
+++
{{ skills() }}
