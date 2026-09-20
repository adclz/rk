# Skills

Skills an agent loads on demand.
Only each skill's `name` and `description` are read up front; the body loads when a task matches, and a skill's `references/` files load only when that skill sends you to them.

`getting-started` is the first one to read.
`cli-*` cover the toolchain, `programming-*` the language and its libraries, `tool-*` the linter and the language server.

Every `iecst` fence is run through the compiler when the site is built; the fence info string says how (`fragment`, `decl`, `continues`, `syntax`, `sketch`, `expect=E0102`), see `crates/doc/src/skills.rs`.

`getting-started`: Install rk and run the loop once — a workspace, a first program, check, test, compile `cli-check`: Check a workspace for diagnostics with `rk check` without producing a binary `cli-compile`: Compile a workspace to a WebAssembly module with `rk compile` `cli-explain`: Look up what a diagnostic code means with `rk explain` `cli-fmt`: Format every .st file in a workspace with `rk fmt` `cli-test`: Compile a workspace and run its {test} functions with `rk test` `programming-config`: Declare how a program actually runs — CONFIGURATION, RESOURCE, TASK, intervals, priorities and VAR_GLOBAL `programming-namespaces`: Organise ST code with NAMESPACE, qualified names, USING directives and the visibility specifiers `programming-oop`: Object-oriented Structured Text — CLASS, METHOD, visibility, EXTENDS, THIS, SUPER and INTERFACE `programming-pointers`: Pointers in Structured Text — REF_TO declarations, REF() to take an address, ^ to dereference, NULL, and the possibly-null analysis (E0902) `programming-st`: Write IEC 61131-3 Structured Text for the `rk` compiler — POUs, VAR sections, types, literals, expressions, statements and pragmas `programming-tests`: Write unit tests in Structured Text — the {test} pragma, Std.Unit's ASSERT/ASSERT_EQ/ASSERT_NEQ, and how to drive a stateful FUNCTION_BLOCK from a test `programming-time`: Date and time in Structured Text — TIME/DATE/DT/TOD literals, their integer encodings, conversions, TO_STRING, and the TON/TOF/TP timers `tool-linter`: The lint rules `rk` applies, what each L-code means and how to configure or silence one `tool-lsp`: Set up the language server so an agent can query types, definitions and diagnostics instead of reading whole files

Reference files (loaded on demand by their skill):

- `cli-compile/references/abi.md`
- `cli-compile/references/sections.md`
- `programming-oop/references/errors.md`
- `programming-oop/references/inheritance.md`
- `programming-oop/references/interfaces.md`
- `programming-st/references/pragmas.md`
- `programming-st/references/runtime.md`
- `programming-st/references/syntax.md`
- `programming-st/references/types.md`
