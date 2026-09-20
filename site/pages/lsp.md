+++
title = "Language server"
description = "What the rk language server brings to your editor."

[extra]
lede = "What the rk language server brings to your editor."
md = "/lsp/index.md"
+++
The language server is a separate binary that ships with the toolchain.

It speaks the Language Server Protocol over stdio, so any editor with an LSP client can use it.

> [!NOTE]
> There is no `rk lsp` subcommand, `rk env` tells you where the server binary is.

## Setup

In VS Code, the extension starts the server for you.

It also adds:

- **rk: Restart Server** and **rk: Stop Server**.
- **rk: Server Status**, which is also shown in the status bar.
- **rk: Run Test**, available above every `{test}` FUNCTION.
- A `rk` build task that compiles a workspace.
- The `rk.path` setting, if `rk` is not on your `PATH`.

For any other editor, see [Other editors](#other-editors).

When the server starts, it parses every `.st` file of the workspace, so all the features work on files you never opened.

> [!IMPORTANT]
> The standard library is found the same way as for the CLI: `RK_STDLIB_PATH`, or beside the server binary.
>
> If it is not found, `Std.*` does not resolve and the server shows a warning with the paths it tried.

A missing `config.toml` is not fatal, it only adds the `E1401` hint to each file.

## Navigation

- **Go to definition** jumps to the declaration of a POU, a type or a method.
  It also crosses into the standard library: on a `TON` instance, it opens `Timers.st`.
- **Go to declaration** jumps to the `VAR` line of a variable, where *Go to definition* lands on its type.
- **Go to type definition** jumps to the type of what is under the cursor.
- **Find all references** lists the declaration and every use, across the whole workspace.
- **Go to implementations** lists the implementers of a `CLASS` or an `INTERFACE`.
  On an interface METHOD, it lists the same method in every POU that implements the interface.
- **Call hierarchy** shows what calls a POU, and what a POU calls.
  It works on FUNCTION, FUNCTION_BLOCK, METHOD and PROGRAM.
- **Document links** turn a `[Name]` written in a comment into a link to the POU it names.

> [!NOTE]
> The call hierarchy reads the same call resolution as the diagnostics, so the overload it shows is the one the compiler picked.

## Reading code

- **Hover** shows the signature, the namespace, and the doc comment written above the declaration.
- **Outline** lists the POUs of a file with their variables and their types.
- **Workspace symbols** is a fuzzy search over the workspace and the standard library at once: `MC` finds `MotorController`.
- **Highlights** mark every occurrence of the name under the cursor in the file.
- **Inlay hints** show the name of the POU on its `END_` keyword, the parameter names on positional arguments, and the element types in struct initializers.
- **Semantic highlighting** colors every name by what it is: a variable of an enum type is a variable, not an enum.
- **Folding** folds each POU and each variable section.

## Writing code

- **Completion** works after a `.`, in a type position, and on a bare identifier.
  It also offers the pragmas after `{`, and the lint rules inside `{allow ...}`.
- **Signature help** shows the parameters of the callee while you type a call.
  An overloaded FUNCTION shows every overload, with the matching one selected.
- **Format document** uses the same engine as `rk fmt`, see [Formatter](/formatter/).
- **Rename** changes every occurrence in the workspace.
- **Quick fixes** come with the diagnostics that have one: `replace '=' with ':='`, or `insert explicit cast 'REAL_TO_INT(r)'`.

> [!IMPORTANT]
> A symbol declared in the standard library cannot be renamed: the edit would only reach the uses and leave the declaration behind.

## Diagnostics

The server reports the same diagnostics as `rk check`, with the same [lint rules](/linter/).

Each diagnostic carries its code, a link to its entry in the [reference](/diagnostics/), and the related locations.

## Tests

- A **run** action is shown above every `{test}` FUNCTION.
- An **implementations** count is shown above every `CLASS` and `INTERFACE`.

> [!NOTE]
> Both are VS Code commands, another editor sees them but cannot run them.

## Debugging

- **Inline values** tell a debugger where each variable is named in the code, so it can show the value next to it.

> [!NOTE]
> The server never sees a runtime value, the debugger provides it.

## Other editors

Any LSP client works, it needs three things:

- The command: the path of the server binary, with no arguments.
- The file pattern: `*.st`.
- The workspace, sent as `workspaceFolders`.

> [!WARNING]
> With only `rootUri`, the server loads nothing: requests return `null` and names from other files do not resolve.

In Neovim:

```lua
vim.lsp.start({
  name = "rk",
  cmd = { "/path/to/vscode-lsp-server" },
  root_dir = vim.fn.getcwd(),
})
```

Logs go to stderr and are filtered by `RUST_LOG`, the default is `warn`.
