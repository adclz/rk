# WASM Code Generation for IEC 61131-3

This crate implements WebAssembly code generation for IEC 61131-3 Structured Text programs.

## Architecture

### HIR → WasmRepr → WASM

The codegen follows a layered approach:

1. **HIR** (High-Level IR from `hir` crate) - Typed, semantic representation
2. **WasmRepr** (`wasm_repr.rs`) - Intermediate representation mapping IEC types to WASM constructs
3. **WASM** (via `wasm-encoder`) - Final WebAssembly binary output

### Key Components

- **`ModuleCodeGen`** - Top-level module builder with scope memoization
- **`FunctionCodegen`** - Per-function code generation with local variable management
- **`WasmRepr`** - Type mapping abstraction (Elementary → ValType, Struct → Memory, etc.)

## Current Implementation Status

### ✅ Implemented

**Core Language Features:**
- **Function signatures** - Correct WASM type signatures from IEC function declarations
- **Local variables** - Proper mapping of Input/InOut/Var/Temp/Output variables to WASM locals
- **Return values** - IEC's "assign to function name" convention handled correctly
- **Variable access** - Reading and writing local variables
- **Assignment statements** - Basic assignment codegen
- **Exports** - Functions exported with their IEC names
- **Type mapping** - Elementary types (INT, REAL, BOOL, etc.) → WASM primitives (i32, f32, f64, i64)

**Expressions:**
- **Literals** - All integer types (SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT), floats (REAL, LREAL), booleans, bit strings (BYTE, WORD, DWORD, LWORD), and inferred literals
- **Arithmetic operators** - Add (+), Subtract (-), Multiply (*), Divide (/), Modulo (MOD) with signed/unsigned variants
- **Comparison operators** - Equal (=), Not Equal (<>), Less Than (<), Less Than or Equal (<=), Greater Than (>), Greater Than or Equal (>=)
- **Boolean operators** - AND, OR, XOR
- **Unary operators** - Negation (-), NOT

**Control Flow:**
- **IF/ELSIF/ELSE** - Full support including nested IF statements and multiple ELSIF branches
- **WHILE loops** - Pre-condition loops with proper block/loop structure
- **REPEAT...UNTIL loops** - Post-condition loops
- **FOR loops** - With optional BY step clause, supports both positive and negative steps
- **EXIT** - Break out of innermost loop
- **CONTINUE** - Jump back to start of innermost loop
- **RETURN** - Early exit from functions
- **CASE statements** - Full support including:
  - Single value matching
  - Multiple values per case (comma-separated)
  - Subrange matching (e.g., 90..100)
  - ELSE clause
  - Nested CASE/IF combinations
- **Nested control flow** - Full support for arbitrarily nested constructs

**Function Calls:**
- **Function-to-function calls** - Full support including:
  - Simple function calls with no parameters
  - Function calls with multiple parameters
  - Chained/nested function calls
  - Function calls in expressions
  - Recursive functions

**Type Conversions:**
- **Implicit type casts** - According to IEC 61131-3 standard (section 6.6.1.6):
  - Integer widening (SINT → INT → DINT → LINT)
  - Integer to float (INT → REAL/LREAL, DINT → LREAL, etc.)
  - Float widening (REAL → LREAL)
  - Unsigned integer conversions (USINT → UINT → UDINT → ULINT)
  - Bit string widening (BOOL → BYTE → WORD → DWORD → LWORD)
  - Automatic cast emission in assignments and function calls

**Composite Data Types:**
- **Arrays** - Full support for arrays (1D and multidimensional):
  - Memory-resident with proper layout
  - Index bounds checking
  - Read and write access
  - Arrays in loops and expressions
  - Arrays of elementary types and structs
- **Structs** - Full support for structured types:
  - Memory-resident with proper alignment
  - Field access (read/write)
  - Nested structs
  - Structs with mixed types (INT, REAL, arrays, etc.)

### 🚧 TODO

**Advanced:**
- [ ] Nested CASE statements (blocked by HIR implementation)
- [ ] Reference types (REF_TO, REFERENCE TO)
- [ ] VAR_IN_OUT parameters (pointer passing)
- [ ] Strings (memory-resident, UTF-8/UTF-16 handling)
- [ ] Function blocks (stateful, memory-resident instances)
- [ ] Classes and methods (inheritance, virtual dispatch)
- [ ] Interfaces (dynamic dispatch)
- [ ] Programs (entry points, cyclic execution)

## Example Usage

```rust
use wasm_codegen::ModuleCodeGen;
use db::RootDatabase;

let db = RootDatabase::default();
// ... add IEC source files to db ...

let mut codegen = ModuleCodeGen::new(&db);
let module = codegen.generate_from_program(program);
let wasm_bytes = module.finish();

// wasm_bytes can now be executed by any WASM runtime
```

## Testing

Run tests:
```bash
cargo nextest run -p wasm_codegen
```

All generated WASM is validated using **wasmtime** (which performs full validation during module creation). Some tests also execute the generated WASM to verify correctness of the output.

## Design Decisions

### Why WasmRepr?

The `WasmRepr` abstraction layer serves several purposes:

1. **Separation of concerns** - Type mapping logic is isolated from instruction emission
2. **Future-proof** - Easy to extend for complex types (arrays, structs, FBs)
3. **Reusability** - Same type mapping used for function signatures, locals, and memory layout
4. **Clarity** - Explicit about how IEC constructs map to WASM primitives vs. memory

### Scope Memoization

Functions are indexed by `ScopeId` to avoid regenerating the same function multiple times. This is important when:
- Multiple files reference the same function
- Incremental compilation is added later
- Function-to-function calls need to resolve indices

### Local Variable Ordering

WASM function locals are indexed sequentially: parameters first, then additional locals. The order is:

1. **Parameters** (Input, InOut) - from `def_map.local_variables` (preserves declaration order)
2. **Return value local** - mapped to the function name
3. **Local variables** (Var, Temp, Output)

This matches IEC 61131-3's scoping semantics while mapping cleanly to WASM.

## Next Steps

The next logical features to implement are:

1. **Literal expressions** - Constants (integers, floats, booleans)
2. **Arithmetic operators** - Basic math (+, -, *, /)
3. **Simple control flow** - IF/ELSE statements
4. **Function calls** - Calling other IEC functions

These building blocks will enable compiling useful IEC programs to WASM.
