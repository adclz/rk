# Licensing

License names used in this document are as per the [SPDX License List](https://spdx.org/licenses/).

The default license for this project is [AGPL-3.0-only](LICENSE).

## Generated modules

[LICENSE-EXCEPTION](LICENSE-EXCEPTION) is an additional permission under AGPL section 7.
Anything the compiler produces from your code, whatever language it was written in and with every custom section it carries, is yours under terms of your choosing; the AGPL imposes no obligation on you or on anyone who receives it.
The builtin bundle and the standard library are also available under Apache-2.0, but for the copy of them inside a generated module the exception governs and Apache-2.0 is only offered in the alternative.

## Apache-2.0

The following directories and their subdirectories are licensed under Apache-2.0, each with its own `LICENSE` file:

```
crates/debug_format/
crates/tree-sitter/
crates/wasm_builtins/
crates/wasm_builtins_generated/
stdlib/
vscode/syntaxes/
```

The builtin bundle and the standard library are copied into every generated module, `debug_format` is what any host reads the module's sections with, and the two grammars, tree-sitter and TextMate, are what any editor parses the language with.

## MIT

The following directories are licensed under the MIT License, being derived from MIT-licensed work:

```
crates/index/
  -> Derived from https://github.com/astral-sh/ruff (ruff_index), after rustc_index.
crates/macros/
  -> Derived from https://github.com/astral-sh/ruff (ruff_macros).
```
