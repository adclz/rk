# Rk 'Rukbat'

> Or __Alpha Sagittarii__  💫

<div align="center" style="font-weight: bold"><strong>Rk</strong> is an IEC-61131-3 Structured Text toolchain that compiles to WebAssembly, focused on strictness and portability.</div>
<br>

 - See [diagnostics](https://rk.clauzeladrien2170.workers.dev/diagnostics) for error codes.

 - See [official documentation](https://rk.clauzeladrien2170.workers.dev) to learn about _**formatter**_ and _**linter**_.

## Why WebAssembly ?

### <img src="site/assets/icons/portable.svg" width="22" height="22" alt=""> **Portable**
One unique binary that can be loaded in any runtime.

### <img src="site/assets/icons/sandboxed.svg" width="22" height="22" alt=""> **Sandboxed**
WebAssembly modules are sandboxed by default, errors are caught but do not propagate to the host.

### <img src="site/assets/icons/host.svg" width="22" height="22" alt=""> **Agnostic host**
In any language, on any [platform](https://withbighair.com/webassembly/2025/05/11/Runtime-choices.html) with a WASM runtime.

## The compiler in a nutshell

### **Expressive**

<img src="site/assets/drawings/expressive.svg" width="380" alt="A namespace holding two overloads of Clamp, and a class implementing an interface">

Namespaces, overloaded functions, classes and interfaces, references and unit tests.

### **Text only**

<img src="site/assets/drawings/text.svg" width="380" alt="Three .st files, their POUs merged into one workspace">

No limitations on how your code can be organized, see [Namespaces](docs/namespaces.md)

### **Highly strict**

<img src="site/assets/drawings/strict.svg" width="380" alt="E0301 on a REAL assigned to an INT, with the cast to write">
  
Built with strictness as its core, with 220+ diagnostics.

### **One core module, with memory dedicated once**

<img src="site/assets/drawings/memory.svg" width="380" alt="The linear memory: builtins, state, bands and strings, sized at compile time">
  
No allocator, no GC, nothing calls `memory.grow`, so the footprint is settled at compile time.

### **Bundled traps**

<img src="site/assets/drawings/trap.svg" width="380" alt="An array store checked against its bounds, raising one exception with its message">
  
Array, subrange bounds, and dereference checks are generated inside the binary, once spotted, they trigger a **WebAssembly exception**.

### **Lightweight**

<img src="site/assets/drawings/pipeline.svg" width="380" alt="From .st through the CST, HIR and MIR to wasm-encoder and a .wasm">
  
Rk does not depend on any compiler backend except wasm-encoder. This makes the compiler very lightweight __(28 MB for the whole executable)__.
  
> [!TIP]
> The CLI provides [binaryen](https://github.com/webassembly/binaryen) as an optional tool you can use to optimize binaries, see [Profiles](docs/profiles.md).

## License

rk is distributed under [AGPL-3.0-only](LICENSE).
For the Apache-2.0 exceptions, the permission that makes every generated module yours,
and commercial licensing, see [LICENSING.md](LICENSING.md).
