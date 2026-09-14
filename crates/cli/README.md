# CLI

The `rk` binary: the compiler's command line.

- `rk check` reports diagnostics for a workspace.
- `rk compile` produces the WebAssembly module.
- `rk test` runs the workspace's `{test}` functions on an in-process wasmtime host.
- `rk fmt` formats the `.st` files.
- `rk explain E0301` describes a diagnostic.
- `rk env` shows the paths it resolved, and where each came from.

```bash
cargo run --bin rk -- check --workspace <path>
```

The library exposes the same commands as `rk::run`, so a tool that wraps this compiler with commands of its own runs the shared ones the same way.
