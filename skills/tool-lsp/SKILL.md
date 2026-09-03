---
name: tool-lsp
description: Set up the language server so an agent can query types, definitions and diagnostics instead of reading whole files. Use when working in a large workspace where reading files wastes context.
---

## Summary

The language server is a separate binary named `vscode-lsp-server`. The name is historical: it is a plain LSP server speaking JSON-RPC over stdio, it takes no arguments, and nothing in the protocol it serves is tied to VSCode. There is no `rk lsp` subcommand; the `rk` CLI does not host the server.

```sh
cargo build --bin vscode-lsp-server
./target/debug/vscode-lsp-server
```

Logs go to stderr, filtered by `RUST_LOG` (default `info` for a debug build, `warn` for a release build). Redirect stderr, it is never part of the protocol stream.

The `initialize` params must carry `workspaceFolders`. At startup the server parses every `.st` file found recursively under them, so all requests answer on files that were never opened. Passing only `rootUri` loads nothing: requests on a file return `null` and cross-file names do not resolve. The standard library resolves the same way as for the CLI: `RK_STDLIB_PATH` from the environment or a `.env` at the workspace root, else beside the server binary. When none is found `Std.*` does not resolve and the server pushes a `window/showMessage` warning naming the paths it tried. A missing `config.toml` is not fatal for the server, it only adds the `E0217` hint diagnostic to each file.

For an agent, this is a cheaper substitute for reading files. `textDocument/documentSymbol` gives the outline of a file (POUs with their kind, their variables with their type) for a fraction of the file's tokens. `textDocument/definition` replaces grepping for a declaration, and it crosses into the library: on a `TON` instance it returns a range inside `stdlib/Timers.st`. `textDocument/hover` returns the signature plus the doc comment written above the declaration, which is usually all that was wanted from opening the file. `workspace/symbol` is a fuzzy search over the workspace and the library at once.

Diagnostics are also served, but for a whole-workspace pass `rk check --output-format json-lines` is simpler than a session; use the LSP diagnostics when a session is already open.

Library files are indexed, not addressable. A `textDocument/*` request whose URI points inside the resolved library directory returns `null`; library symbols are only reachable as `definition`/`declaration` targets and through `workspace/symbol`.

## Usage

`textDocument/documentSymbol` Nested symbols. POUs, programs, top-level namespaces and configurations, each with the declared variables as children; `detail` carries `FUNCTION`, `FUNCTION_BLOCK`, `PROGRAM` on the POU and the resolved type name on a variable.

`textDocument/definition` One `Location` spanning the whole declaration of the resolved item, its POU for a type reference or a call.

`textDocument/declaration` The declaration behind an expression. On a variable use it lands on the `VAR` line, where `definition` lands on the type.

`textDocument/references` Declaration and uses, across every workspace file.

`textDocument/hover` Markdown, an `iecst` fence with the signature (`(VAR) total: REAL`, `FUNCTION Add: INT`), the qualified namespace when there is one, and the preceding doc comment below a rule.

`workspace/symbol` Fuzzy, capped at 128 results, searches workspace files and the library index. An empty query returns nothing. `containerName` is the namespace, so a library hit shows as `Std.Timers`.

`textDocument/diagnostic` and `workspace/diagnostic` Pull diagnostics, one file or the whole workspace. Each item carries the `code` (`E0301`), a `codeDescription.href` to the online explanation, and `relatedInformation`.

`textDocument/codeAction` Quick fixes of the diagnostics overlapping the range. Syntax fixes carry a real edit (`replace '=' with ':='`). The implicit-cast suggestion (`insert explicit cast 'REAL_TO_INT(r)'`) carries an empty `changes` map, it is a title only, so never apply an action without checking its edit.

`textDocument/formatting` One `TextEdit` replacing the whole file. Same engine as `rk fmt`.

`textDocument/rename` A `WorkspaceEdit` over every occurrence.

`textDocument/foldingRange` One region per POU and per variable section.

`textDocument/inlayHint` End-marker labels (`FUNCTION_BLOCK Motor` on `END_FUNCTION_BLOCK`), parameter names on positional call arguments, and element types in struct initializers.

`textDocument/semanticTokens/full` and `/range` The legend has 9 token types (`namespace`, `function`, `method`, `interface`, `class`, `struct`, `enum`, `enumMember`, `event`) and no modifiers. Only declarations and uses of those are emitted; keywords and literals are not, an editor colours them with its own grammar.

`textDocument/documentLink` Links from `[Name]` bracket references inside comments to the POU they name.

`textDocument/implementation` Implementers of a `CLASS` or an `INTERFACE`, as `LocationLink`s. Returns `null` on anything else.

`textDocument/codeLens` Two lenses only: an implementation count on a `CLASS`/`INTERFACE`, and a run action on a `{test}` POU. Both are `Command`s addressed to VSCode extension commands (`rk.showImplementations`, `rk.runTest`), useless to another client.

`textDocument/signatureHelp` The callee's label with input/output/inout parameters and the active one. It resolves inside `FUNCTION`, `FUNCTION_BLOCK` and `METHOD` bodies only; inside a `PROGRAM` body it returns `null`.

`textDocument/completion` Trigger characters are `.`, `#` and `(`. Member completion after `.` and type completion in a declaration position work; a bare identifier prefix in a body generally returns nothing. It is shaped for an editor's cursor, not a good agent tool.

## Keeping the server in sync

The server answers from its own copy of the files, not from disk. After writing a file, tell it, otherwise every later answer is stale.

`workspace/didChangeWatchedFiles` The simplest path when the agent edits files on disk. Types `1` created, `2` changed and `3` deleted are all honoured, whether or not the client declared `didChangeWatchedFiles` dynamic registration.

`textDocument/didOpen` then `textDocument/didChange` For a buffer that is not on disk. A single `contentChanges` entry holding the full new text is accepted. `didOpen` also registers a `.st` file that lives outside the workspace folders; on a library file it is ignored.

The server may send `client/registerCapability` and `workspace/diagnostic/refresh` in the other direction. It does not wait for the answers, a minimal driver can ignore them.

## Other clients

Any LSP client works. The registration needs three things: the command (the binary path, no arguments), the file pattern `*.st`, and a root directory containing the workspace, sent as `workspaceFolders`. In Neovim:

```lua
vim.lsp.start({
  name = "rk",
  cmd = { "/path/to/vscode-lsp-server" },
  root_dir = vim.fn.getcwd(),
})
```

Requests that are not registered are refused with JSON-RPC `-32601`, not with an empty answer. That is the case for `textDocument/typeDefinition`, `documentHighlight`, `prepareRename`, `rangeFormatting`, `selectionRange` and call hierarchy, none of which the server implements.

The VSCode extension adds what the protocol does not carry: it copies the built binary to `vscode/server/bin/`, owns the two code lens commands, and consumes a custom `rk/serverStatus` notification for its status bar. Another client sees that notification and can drop it.
