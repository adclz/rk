# RK LSP

**$RK** is a compiler front-end and Language Server Protocol (LSP) implementation for Structured Text (.st), inspired by the design of demand-driven compilers.

Although still in its early stages, $RK already includes:

- ⚡ A type checker.
- 🧹 A code formatter.
- 🧠 A LSP server.

The current list of features is non-exhaustive and is likely to change until the first release.

## Summary

- [Technical overview](#technical-overview)
    - [Architecture](#architecture)
    - [Design philosophy](#design-philosophy)
- [LSP](#lsp)
    - [Document Symbols](#document-symbols)
    - [Inlay Hints](#inlay-hints)
    - [Semantic tokens](#semantic-tokens)
    - [Folding Ranges](#folding_ranges)
    - [Hover](#hover)
        - [Comment Index](#comment-index)
    - [GoToDefinition/Declaration](#go-to-definition)
    - [Implementations](#implementations)
    - [Completions](#completions)
        - [Static snippets](#static-snippets)
        - [Signatures](#signatures)
        - [Fly imports](#fly-imports)
- [Namespaces and files](#namespaces-and-files)
- [Type checker](#type-checker)
    - [Namespaces and files](#namespaces-and-files)
    - [Type Checking](#type-checking)
    - [Fuzzy search recovery](#fuzzy-search-recovery)
- [Formatter](#formatter)
    - [Semicolons](#semi-colons)
    - [Inlining Lists](#inlining-lists)

## Technical overview

### 🧩 Architecture

$RK belongs to the family of demand-driven (or query-based) compilers, a design popularized by Rust’s compiler infrastructure.
Instead of following a traditional monolithic compiler pipeline, $RK is structured around **CST / AST / HIR / MIR** layers and follows **Data-Oriented** Programming principles.

Its architecture emphasizes:

 - **Immutable, algebraic data structures**

 - **Composable design**, where each stage has a clear, isolated responsibility

 - **Entity–Component System** ([ECS](https://github.com/SanderMertens/ecs-faq?tab=readme-ov-file#what-is-ecs)) organization, where every code element is an independent entity aware only of its local scope and its relations via queries

 - **Incremental computation**, powered by [salsa](https://salsa-rs.netlify.app/)

This makes $RK conceptually similar to tools like [ruff](https://docs.astral.sh/ruff/) or [rust-analyzer](https://rust-analyzer.github.io/).

### ⚙️ Design Philosophy

$RK performs on-demand computation: nothing is evaluated unless it’s explicitly required.
This design allows it to stay responsive even during rapid code changes—whether files are saved or not—by caching results and reusing them efficiently between requests.

While many details are omitted here (such as fault-tolerant lossless parsing, interning, arena allocation, request cancellation, or parallelism).

## LSP

### Document Symbols

Documents symbols provide a list of symbols for the IDE's code outline, which gives a high-level overview of the source code.

This is a very common feature that many language extensions provide, but in our case $RK tries to provide more information about certain symbols, such as return type of FUNCTIONs and METHODs, or aliases.

For example here, the variable **Level** of **CHECK_SAFETY** function is refereing the enum _Room_Safety_.

$RK will display both the name of the **type** (_Room_Safety_) this variable refers to, and also the **type** itself (in this case, _Enum_).

Document symbols are also used to fullfill the [Workspace Symbols LSP request](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#workspace_symbol).

![document_symbols](/assets/document_symbols.png)

### Inlay Hints

**Inlay Hints** are additional intra-text-information, $RK uses inlay hint in two places:

 - As the end of a POU or NAMESPACE definition in order to quickly reminds the user in which contexts the end of a POU or NAMESPACE belongs to.
 - Struct or function call parameters

![inlay_hints](/assets/inlay_hints.png)

### Hover

$RK allows hovering of all items that can have a semantic meaning.
Hovering in item will also highligh it's definition or definition if it's available on the current window.

![hover_simple](/assets/hover_simple.mp4)

### Comment Index

$RK maintains a comment index that is lazily computed when the user is hovering a type or variable declaration.

A comment will be displayed for a given item if a it's declared on top or at the right of it.

If the item has both top and right comments, the top one will have the priority.

Comments support markdown and the following syntax stules are allowed:
 - Multiline C-style comment /* ... */
 - Multiline Pascal-Style comment (* ...  *)
 - Single line // ...

![hover_comment_simple](/assets/hover_comment_simple.mp4)

$RK makes a difference between a comment that is used for a type definition and a comment for a type reusing it.

![hover_comment_variable](/assets/hover_comment_variable.mp4)

### Implementations

In the case of Object Oriented programming features, any FUNCTION_BLOCK, CLASS, or INTERFACE that is extends/implemented by another POU will have a [Code lens](https://code.visualstudio.com/blogs/2017/02/12/code-lens-roundup) that shows how many POUs are using it.

Clicking on that lens will show the locations of those POUs are located.

![implementations](/assets/implementations.mp4)

### Semantic Tokens 

**Semantic tokens** are enhancing syntax highlighting by providing colors according to the [semantic definition of a type](https://code.visualstudio.com/api/language-extensions/semantic-highlight-guide).
They overcome the limitations of a classic parser or text mate grammar that can not achieve complete context awareness.

In the following example, all references to both fb0 and cl0 are blu

![tokens_before](/assets/semantic_tokens_before.png)

With semantic tokens enabled, the colors of variable types match their actual definitions.
References to variables used in statements will also match the color of their definitions.

![tokens_after](/assets/semantic_tokens_after.png)


### Folding Ranges

Folding ranges allow the user to collapse code regions in the IDE.

When folding items that have a semantic meaning, $RK will try preserve the name of the item being folded.

![folding_ranges](/assets/folding_ranges.mp4)

### GoToDefinition/Declaration

Going to definition or declaration will move the cursor to where a type is defined.

The difference between definitions and declarations is that definiton are pure type definitions, whereas declarations are used for variables

### Completions 

#### Static Snippets

By static snippets we mean completions that always remain the same, e.g declaring a new POU or a variable section.

However $RK will try to show them only at locations where it makes sense to have them.

In the following example, we're declaring NAMESPACES and a FUNCTION.

- It makes sense to suggest a NAMESPACE when there's no context, or within an already existing NAMESPACE.
- It makes sense to suggest a FUNCTION inside a NAMESPACE.
- However, it does not make sense to suggest a NAMESPACE inside a FUNCTION.

Therefore typing 'N' inside FUNCTION should give no suggestions.

![static_snippets](/assets/completion_static_snippets.mp4)

#### Signatures

When suggesting a function or method, $RK will automatically provide a signature snippet that includes all parameters of that function or method.

If the function or method has more than 5 parameters, $RK will try to split them into multiple lines for better readability.

Signature completions have (CALL) as a label to indicate they are callable items.

![signature](/assets/completion_signature.mp4)

#### Fly imports

Since $RK supports multiple files and NAMESPACES, it can automatically suggest imports when the user is trying to use an item that is not yet imported in the current scope.

Fly imports completions are labeled with (USING) to indicate that selecting this completion will also add an import statement.

![fly_imports](/assets/completion_fly_imports.mp4)

## Type Checker

$RK has a type checker that runs alongisde the LSP server.

The type checker currently has 80 error types.

### Namespaces and files

$RK supports NAMESPACEs and automatically merges them accross files.
That means declaring a NAMESPACE in multiple files will result in a single NAMESPACE that contains all items declared in those files.

Therefore, duplicates across files are detected and reported by the type checker.

![namespace_dups](/assets/type-check-dups-in-namespaces.mp4)

### Type Checking

$RK listens to all file events coming from the LSP client, even if the files are not saved on disk.

That means the type checker fires on every keystroke:

![type_check_simple](/assets/type-check-simple.mp4)

![type_check_inherit](/assets/type-check-inherit.mp4)

Often, the type checker will try to provide additional informations on why an error happened either by:
 - Displaying related informations about the items affected by an error.
 - A note on why this error occured and how the user should potentially fix it.

 ### Fuzzy search recovery

For missing items, $RK will fuzzy search items available in scope and returns those who match the missing item's name.

![type_check_fuzzy](/assets/type-check-fuzzy.mp4)

This also works for STRUCT fields and function calls:

![type_check_fuzzy_struct](/assets/type-check-recovery-struct.png)
![type_check_fuzzy_params](/assets/type-check-recovery-params.png)

## Formatter

$RK has a built-in formatter that can be called at anytime by the LSP client.

However, it requires that the file has no syntax errors.

![formatter](/assets/formatter.mp4)

### Semicolons

The parser **tolerates** missing semicolons ';' at the end of items, but it keeps compatibility with the IEC standard by allowjng them at the right locations.

However, the formatter will automatically add missing semi-colons if it sees they are missing.

![formatter](/assets/formatter_semi_colons.mp4)

### Inlining lists

The formatter will expand lists of parameters or values depending if a new line is present in the list.

In this example, it will keep everything in a single line.

![formatter](/assets/formatter_single_line.mp4)

But if a new line is present, it will add a line for each parameter: 


![formatter](/assets/formatter_multi_line.mp4)