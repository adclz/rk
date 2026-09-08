---
name: tool-lsp
description: Set up the language server so an agent can query types, definitions and diagnostics instead of reading whole files. Use when working in a large workspace where reading files wastes context.
---

## Summary

The language server is a separate binary, downloaded per platform with the toolchain.
It speaks JSON-RPC over stdio and takes no arguments.
There is no `rk lsp` subcommand; the `rk` CLI does not host the server.
It is launched, never built: `rk env` names the copy that goes with this `rk` on its `lsp` row.

```sh
rk env --output-format json-lines | jq -r 'select(.key == "lsp") | .value'
```

Logs go to stderr, filtered by `RUST_LOG`; the default is `warn`, so a session is silent unless asked.
Redirect stderr, it is never part of the protocol stream.

The `initialize` params must carry `workspaceFolders`.
At startup the server parses every `.st` file found recursively under them, so all requests answer on files that were never opened.
Passing only `rootUri` loads nothing: requests on a file return `null` and cross-file names do not resolve.
The standard library resolves the same way as for the CLI: `RK_STDLIB_PATH` from the environment or a `.env` at the workspace root, else beside the server binary.
When none is found `Std.*` does not resolve and the server pushes a `window/showMessage` warning naming the paths it tried.
A missing `config.toml` is not fatal for the server, it only adds the `E0217` hint diagnostic to each file.

For an agent, this is a cheaper substitute for reading files.
`textDocument/documentSymbol` gives the outline of a file (POUs with their kind, their variables with their type) for a fraction of the file's tokens.
`textDocument/definition` replaces grepping for a declaration, and it crosses into the library: on a `TON` instance it returns a range inside `stdlib/Timers.st`.
`textDocument/hover` returns the signature plus the doc comment written above the declaration, which is usually all that was wanted from opening the file.
`workspace/symbol` is a fuzzy search over the workspace and the library at once.

Diagnostics are also served, but for a whole-workspace pass `rk check --output-format json-lines` is simpler than a session; use the LSP diagnostics when a session is already open.

Library files are indexed, not addressable.
A `textDocument/*` request whose URI points inside the resolved library directory returns `null`; library symbols are only reachable as `definition`/`declaration` targets and through `workspace/symbol`.

## Usage

`textDocument/documentSymbol` Nested symbols.
POUs, programs, top-level namespaces and configurations, each with the declared variables as children; `detail` carries `FUNCTION`, `FUNCTION_BLOCK`, `PROGRAM` on the POU and the resolved type name on a variable.

`textDocument/definition` One `Location` on the NAME of the resolved item, its POU for a type reference or a call.
It spans the identifier, not the whole declaration: a range covering the POU made an editor conclude the cursor was already there and show references instead of jumping.

`textDocument/declaration` The declaration behind anything that has one: variables and fields, and also POUs, methods, enum variants and namespaces.
On a variable use it lands on the `VAR` line, where `definition` lands on the type.

`textDocument/typeDefinition` The TYPE of what the cursor is on, where `definition` gives its declaration.
On `m : Mode` it lands on `Mode`; on a call it lands on the return type's declaration.
An elementary type is written nowhere, so `INT` answers `null` rather than falling back to the variable.

`textDocument/references` Declaration and uses, across every workspace file.

`textDocument/documentHighlight` The same occurrences narrowed to ONE document, which is what the editor paints as the cursor moves.
Every hit is `kind: 1` (text): telling a read from a write is inference's answer, and this request is not the place to take a second opinion on it.

`textDocument/hover` Markdown, an `iecst` fence with the signature (`(VAR) total: REAL`, `FUNCTION Add: INT`), the qualified namespace when there is one, and the preceding doc comment below a rule.

`workspace/symbol` Fuzzy, capped at 128 results, searches workspace files and the library index.
METHODs are included alongside POUs, programs and types.
Results are RANKED, best first: an exact match, then a prefix, then a case-insensitive one, then a subsequence, and shorter names before longer ones at equal rank.
Subsequence matching is kept deliberately, as it is what makes `MC` find `MotorController`; ranking is what stops it burying the answer.
An empty query returns nothing.
`containerName` is the namespace, so a library hit shows as `Std.Timers`.

`textDocument/diagnostic` and `workspace/diagnostic` Pull diagnostics, one file or the whole workspace.
Each item carries the `code` (`E0301`), a `codeDescription.href` to the online explanation, and `relatedInformation`.
Lints are included on the recommended set without any configuration, the same default `rk check` applies; a `[linter]` section in `config.toml` changes which rules run, it is not what turns them on.

`textDocument/codeAction` Quick fixes of the diagnostics overlapping the range.
Syntax fixes carry a real edit (`replace '=' with ':='`).
The implicit-cast suggestion (`insert explicit cast 'REAL_TO_INT(r)'`) carries the edit its title promises, replacing the offending expression with the call.
It is offered only where a conversion can be called: an initializer, an enum value and a subrange bound have no call site, so no fix is attached there.

`textDocument/formatting` One `TextEdit` replacing the whole file.
Same engine as `rk fmt`.

`textDocument/rename` A `WorkspaceEdit` over every occurrence.
A symbol declared in the library is refused rather than renamed: the edit could only reach the uses, leaving the declaration behind and the workspace on `E0210`.

`textDocument/foldingRange` One region per POU and per variable section.

`textDocument/inlayHint` End-marker labels (`FUNCTION_BLOCK Motor` on `END_FUNCTION_BLOCK`), parameter names on positional call arguments, and element types in struct initializers.

`textDocument/inlineValue` Where a debugger should show what a variable holds, while a session is stopped.
The server never sees a runtime value: it answers with the RANGES that name a variable and the name to look up, and the debugger supplies the value for the frame.
Every answer is an `InlineValueVariableLookup` with `caseSensitiveLookup: false`, because IEC folds case and `Motor` and `motor` are one name.
Declarations and bare uses are offered, up to and including the stopped line; nothing below it, where a value would be last scan's.
A FIELD step is deliberately not offered on its own: in `g.out` the lookup is `g`, since a bare `out` is not a name the debugger's scope holds and the path that would reach it is the runtime's shape, not the source's.
Note that `lsp-types` declares this request's result as a single value where the specification says an array; the server answers with the array.

`textDocument/semanticTokens/full` and `/range` The legend has 13 token types (`namespace`, `function`, `method`, `interface`, `class`, `struct`, `enum`, `enumMember`, `event`, `variable`, `parameter`, `property`, `type`) and no modifiers.
Every name a body writes carries one, and it is the token for what the name IS rather than what it is OF: a variable of enum type is a `variable`, not an `enum`.
That covers declarations and uses, call sites (the callee, so an invoked FB instance is a `variable` and the block it runs is not named), named and output arguments (`p := v`, `o => v`), enum variants and struct fields where they are DECLARED, and each segment of a path separately, so `a.b.c` is three tokens and not one.
Keywords and literals are not emitted, an editor colours them with its own grammar.

`textDocument/prepareCallHierarchy`, `callHierarchy/incomingCalls` and `callHierarchy/outgoingCalls` What calls this, and what this calls.
Prepare answers on a `FUNCTION`, a `FUNCTION_BLOCK`, a `METHOD` or a `PROGRAM`, at its declaration or at a call naming it; on a call site the hierarchy is the callee's.
Incoming scans every body in the workspace, outgoing reads one body.
Both read the call plan inference recorded when it checked the call, so the overload they show is the overload the diagnostics agree with.
Two calls to one callee are one entry with two `fromRanges`, not two entries.
Invoking an FB instance counts as a call to the BLOCK, which is what runs.
A `PROGRAM` answers outgoing calls and never appears as an incoming one: a `TASK` schedules it, no body invokes it.

`textDocument/documentLink` Links from `[Name]` bracket references inside comments to the POU they name.

`textDocument/implementation` Implementers of a `CLASS` or an `INTERFACE`, as `LocationLink`s.
On an interface METHOD it returns the same-named method of every POU implementing that interface, which is the question a reader actually has there.
Returns `null` on anything else.

`textDocument/codeLens` Two lenses only: an implementation count on a `CLASS`/`INTERFACE`, and a run action on a `{test}` POU.
Both are `Command`s addressed to VSCode extension commands (`rk.showImplementations`, `rk.runTest`), useless to another client.

`textDocument/signatureHelp` The callee's label with input/output/inout parameters and the active one.
It resolves in every body: `FUNCTION`, `FUNCTION_BLOCK`, `METHOD` and `PROGRAM`.
An overloaded callee returns every overload, with `activeSignature` on the one whose parameters the written arguments fit.
Trigger characters are `(` and `,`, and the popup is retained on `,` and `)` so it survives the argument being typed.

`textDocument/completion` Trigger characters are `.`, `#`, `(`, `{` and `'`.
Member completion after `.`, type completion in a declaration position, and a bare identifier prefix in a body all work.
A VAR section offers what a declaration takes there, including the visibility keywords in a POU header and `AT` in the sections that write a location.
`{` offers the pragmas legal at that place, and `{allow ...}` offers the lint rules it can silence.
It is still shaped for an editor's cursor rather than for an agent: it answers about one position, where `documentSymbol` and `workspace/symbol` answer about a file or a workspace.

## Keeping the server in sync

The server answers from its own copy of the files, not from disk.
After writing a file, tell it, otherwise every later answer is stale.

`workspace/didChangeWatchedFiles` The simplest path when the agent edits files on disk.
Types `1` created, `2` changed and `3` deleted are all honoured, whether or not the client declared `didChangeWatchedFiles` dynamic registration.

`textDocument/didOpen` then `textDocument/didChange` For a buffer that is not on disk.
A single `contentChanges` entry holding the full new text is accepted.
`didOpen` also registers a `.st` file that lives outside the workspace folders; on a library file it is ignored.

The server may send `client/registerCapability` and `workspace/diagnostic/refresh` in the other direction.
It does not wait for the answers, a minimal driver can ignore them.

## Other clients

Any LSP client works.
The registration needs three things: the command (the binary path, no arguments), the file pattern `*.st`, and a root directory containing the workspace, sent as `workspaceFolders`.
In Neovim:

```lua
vim.lsp.start({
  name = "rk",
  cmd = { "/path/to/vscode-lsp-server" },
  root_dir = vim.fn.getcwd(),
})
```

Requests that are not registered are refused with JSON-RPC `-32601`, not with an empty answer.
That is the case for `prepareRename`, `rangeFormatting`, `onTypeFormatting`, `selectionRange`, `linkedEditingRange`, `workspace/willRenameFiles` and `moniker`, none of which the server implements.
Type hierarchy is the one gap that is not a choice: the protocol crate the server is built on predates the 3.17 `typeHierarchyProvider` capability and has no way to advertise it, so the request would be refused however it were answered.
Call hierarchy, type definition, document highlight and inline values are all implemented.

The VSCode extension adds what the protocol does not carry: it copies the built binary to `vscode/server/bin/`, owns the two code lens commands, and consumes a custom `rk/serverStatus` notification for its status bar.
Another client sees that notification and can drop it.
