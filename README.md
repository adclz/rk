# RK LSP

**$RK** is a compiler front-end and Language Server Protocol (LSP) implementation for Structured Text (.st), inspired by demand-driven compiler design.

Although still in its early stages, $RK already provides:

- ⚡ A type checker
- 🧹 A code formatter
- 🧠 An LSP server

The feature list is non-exhaustive and subject to change before the first release.

## Summary

- [Technical overview](#technical-overview)
    - [Architecture](#architecture)
    - [Design philosophy](#design-philosophy)
- [LSP](#lsp)
    - [Document Symbols](#document-symbols)
    - [Inlay Hints](#inlay-hints)
    - [Semantic tokens](#semantic-tokens)
    - [Folding Ranges](#folding-ranges)
    - [Hover](#hover)
        - [Comment Index](#comment-index)
    - [GoToDefinition/Declaration](#gotodefinition-declaration)
    - [Implementations](#implementations)
    - [Completions](#completions)
        - [Static snippets](#static-snippets)
        - [Signatures](#signatures)
        - [Fly imports](#fly-imports)
- [Type checker](#type-checker)
    - [Namespaces and files](#namespaces-and-files)
    - [Type Checking](#type-checking)
    - [Fuzzy search recovery](#fuzzy-search-recovery)
- [Formatter](#formatter)
    - [Semicolons](#semicolons)
    - [Multiline](#multiline)

## Technical overview

### Architecture

$RK belongs to the family of demand-driven (or query-based) compilers, a design popularized by Rust’s compiler infrastructure.
Instead of following a traditional monolithic compiler pipeline, $RK is structured around **CST / AST / HIR / MIR** layers and follows **Data-Oriented** Programming principles.

Its architecture emphasizes:

 - **Immutable, algebraic data structures**

 - **Composable design**, where each stage has a clear, isolated responsibility

 - **Entity–Component System** ([ECS](https://github.com/SanderMertens/ecs-faq?tab=readme-ov-file#what-is-ecs)) organization, where every code element is an independent entity aware only of its local scope and its relations via queries

 - **Incremental computation**, powered by [salsa](https://salsa-rs.netlify.app/)

This makes $RK conceptually similar to tools like [ruff](https://docs.astral.sh/ruff/) or [rust-analyzer](https://rust-analyzer.github.io/).

### Design Philosophy

$RK performs on-demand computation: nothing is evaluated unless explicitly required.
This design enables it to remain responsive during rapid code changes—whether files are saved or not—by caching results and efficiently reusing them between requests.

This document focuses on features. A technical documentation will cover omitted details such as fault-tolerant lossless parsing, interning, arena allocation, request cancellation, and parallelism.

## LSP

### Document Symbols

Document symbols provide a list of symbols in the IDE's code outline, offering a high-level overview of the source code.

While many language extensions provide this feature, $RK goes further by displaying additional information about certain symbols, such as the return type of FUNCTIONs and METHODs, or type aliases.

For example, the variable **Level** in the **CHECK_SAFETY** function references the enum _Room_Safety_.

$RK displays both the name of the **type** that the variable refers to (_Room_Safety_) and the **kind of type** itself (in this case, an _Enum_).

Document symbols also fulfill the [Workspace Symbols LSP request](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#workspace_symbol).

![document_symbols](/assets/document_symbols.png)

### Inlay Hints

**Inlay Hints** provide additional inline information. $RK uses them in two places:

 - At the end of POU or NAMESPACE definitions to remind the user which context they belong to
 - For struct and function call parameters

![inlay_hints](/assets/inlay_hints.png)

### Hover

$RK allows hovering over all items that have semantic meaning.
When you hover over an item, $RK highlights its definition if it's visible in the current window.

![hover_simple](/assets/hover_simple.mp4)

### Comment Index

$RK maintains a comment index that is lazily computed when the user hovers over a type or variable declaration.

A comment is displayed for an item if it's declared above or to the right of it.

If an item has both top and right comments, the top one takes priority.

Comments support markdown and the following styles:
 - Multiline C-style comment `/* ... */`
 - Multiline Pascal-style comment `(* ... *)`
 - Single-line comment `// ...`

![hover_comment_simple](/assets/hover_comment_simple.mp4)

$RK distinguishes between comments used for a type definition and comments for a type that reuses it.

![hover_comment_variable](/assets/hover_comment_variable.mp4)

### Implementations

In the case of Object Oriented programming features, any FUNCTION_BLOCK, CLASS, or INTERFACE that is extends/implemented by another POU will have a [Code lens](https://code.visualstudio.com/blogs/2017/02/12/code-lens-roundup) that shows how many POUs are using it.

Clicking on that lens will show the locations of those POUs are located.

![implementations](/assets/implementations.mp4)

### Semantic Tokens 

**Semantic tokens** enhance syntax highlighting by providing colors based on the [semantic definition of a type](https://code.visualstudio.com/api/language-extensions/semantic-highlight-guide).
They overcome the limitations of traditional parsers or TextMate grammars, which cannot achieve complete context awareness.

In the following example, all references to both `fb0` and `cl0` are blue.

![tokens_before](/assets/semantic_tokens_before.png)

With semantic tokens enabled, the colors of variable types match their actual definitions.
References to variables in statements also match the color of their definitions.

![tokens_after](/assets/semantic_tokens_after.png)


### Folding Ranges

Folding ranges allow users to collapse code regions in the IDE.

When folding items with semantic meaning, $RK preserves the name of the item being folded.

![folding_ranges](/assets/folding_ranges.mp4)

### GoToDefinition Declaration

Goto definition or declaration moves the cursor to where a type is defined.

The difference between definitions and declarations is that definitions are pure type definitions, while declarations are used for variables.

### Completions 

#### Static Snippets

Static snippets are completions that always remain the same, such as declaring a new POU or variable section.

However, $RK only shows them at locations where they make sense.

In the following example, we're declaring NAMESPACES and a FUNCTION:

- It makes sense to suggest a NAMESPACE when there's no context or within an existing NAMESPACE.
- It makes sense to suggest a FUNCTION inside a NAMESPACE.
- It does not make sense to suggest a NAMESPACE inside a FUNCTION.

Therefore, typing 'N' inside a FUNCTION yields no suggestions.

![static_snippets](/assets/completion_static_snippets.mp4)

#### Signatures

When suggesting a function or method, $RK automatically provides a signature snippet that includes all parameters.

If the function or method has more than 5 parameters, $RK splits them across multiple lines for better readability.

Signature completions are labeled `(CALL)` to indicate they are callable items.

![signature](/assets/completion_signature.mp4)

#### Fly imports

Since $RK supports multiple files and NAMESPACES, it can automatically suggest imports when you try to use an item that isn't yet imported in the current scope.

Fly imports completions are labeled with `(USING)` to indicate that selecting this completion also adds an import statement.

![fly_imports](/assets/completion_fly_imports.mp4)

## Type Checker

$RK includes a type checker that runs alongside the LSP server.

The type checker currently supports 80 error types.

### Namespaces and files

$RK supports NAMESPACEs and automatically merges them across files.
This means declaring a NAMESPACE in multiple files results in a single NAMESPACE containing all items from those files.

Therefore, duplicates across files are detected and reported by the type checker.

![namespace_dups](/assets/type-check-dups-in-namespaces.mp4)

### Type Checking

$RK listens to all file events from the LSP client, even if files aren't saved to disk.

This means the type checker runs on every keystroke:

![type_check_simple](/assets/type-check-simple.mp4)

![type_check_inherit](/assets/type-check-inherit.mp4)

Often, the type checker provides additional information about errors by:
 - Displaying related details about the items affected by an error
 - Providing notes on why the error occurred and how to fix it

### Fuzzy search recovery

For missing items, $RK performs a fuzzy search across available items in scope and returns those that match the missing item's name.

![type_check_fuzzy](/assets/type-check-fuzzy.mp4)

This also works for STRUCT fields and function calls:

![type_check_fuzzy_struct](/assets/type-check-recovery-struct.png)
![type_check_fuzzy_params](/assets/type-check-recovery-params.png)

## Formatter

$RK includes a built-in formatter that can be invoked by the LSP client at any time.

However, it requires that the file has no syntax errors.

![formatter](/assets/formatter.mp4)

### Semicolons

The parser **tolerates** missing semicolons at the end of items while maintaining compatibility with the IEC standard by allowing them at the correct locations.

The formatter automatically adds missing semicolons when needed.

![formatter](/assets/formatter_semi_colons.mp4)

### Multiline

The formatter expands or collapses lists of parameters or values based on whether a new line is present.

In this example, everything stays on a single line:

![formatter](/assets/formatter_single_line.mp4)

If a new line is present, the formatter places each parameter on its own line:

![formatter](/assets/formatter_multi_line.mp4)