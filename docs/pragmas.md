# Pragmas

Only these exist; anything else in braces is a syntax error,
so `{attribute '…'}` from other toolchains cannot be copied in.

- `{test}` marks a `FUNCTION` the test runner calls.

- `{once}` marks a `FUNCTION`, `FUNCTION_BLOCK` or `METHOD` that should be called at most once per body.

- `{warn = 'message'}` and `{info = 'message'}` attach a diagnostic to every call site of the POU.

- `{allow 'rule' 'rule'}` silences lint rules: above a POU for the whole POU, as a statement for the next statement.


- `{export}` marks a `FUNCTION` a host can call, see [WASM ABI](wasm-abi.md).

- `{extern 'module' 'name'}` declares a `FUNCTION` as a WASM import; the declaration is the signature.


- `{wasm 'instruction' (params a b) (result r)}` is a statement that emits one WASM instruction on the named operands.
