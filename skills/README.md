# Skills

Skills an agent loads on demand.
Only each skill's `name` and `description` are read up front; the body loads when a task matches, and a skill's `references/` files load only when that skill sends you to them.

`getting-started` is the first one to read.
`cli-*` cover the toolchain, `programming-*` the language and its libraries, `tool-*` the linter and the language server.

Every `iecst` fence is run through the compiler when the site is built; the fence info string says how (`fragment`, `decl`, `continues`, `syntax`, `sketch`, `expect=E0102`), see `crates/doc/src/skills.rs`.


Reference files (loaded on demand by their skill):

- `programming-oop/references/errors.md`
- `programming-oop/references/inheritance.md`
- `programming-oop/references/interfaces.md`
- `programming-st/references/pragmas.md`
- `programming-st/references/syntax.md`
- `programming-st/references/types.md`
