//! Compile `wasm_builtins` to `wasm32-unknown-unknown`, parse it, and emit
//! this crate's `src/lib.rs` - a static table mapping each export's name to
//! its function-body bytes plus the indices of every function it
//! transitively calls. `wasm_codegen` consumes the result at runtime to
//! graft math intrinsics into IEC outputs without host imports.
//!
//! The generated `src/lib.rs` is gitignored and should never be hand-edited.

use std::path::PathBuf;
use std::process::Command;

use wasmparser::{FuncType, Operator, Parser, Payload, TypeRef, ValType};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let builtins_crate = workspace_root.join("crates").join("wasm_builtins");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    println!(
        "cargo:rerun-if-changed={}",
        builtins_crate.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        builtins_crate.join("Cargo.toml").display()
    );
    println!("cargo:rerun-if-changed=build.rs");

    // Use a dedicated target dir so the recursive cargo invocation doesn't
    // contend with the parent build for the workspace lock.
    let target_dir = out_dir.join("builtins-target");
    // Shrink the libm shadow stack from the default 1 MiB to 8 KiB. This
    // collapses the bundle's memory footprint from 17 pages to 1 page -
    // the .cargo/config.toml in `crates/wasm_builtins` would do the same
    // thing but isn't on the discovery path when build.rs runs from
    // `crates/wasm_codegen`, so we set it here explicitly.
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args([
        "build",
        "--release",
        "--target=wasm32-unknown-unknown",
        "--manifest-path",
    ])
    .arg(builtins_crate.join("Cargo.toml"))
    .arg("--target-dir")
    .arg(&target_dir)
    // `RUSTFLAGS` doesn't propagate from a parent cargo (we are one,
    // since this is a build.rs). `--config` injects the same setting
    // explicitly and survives the recursive invocation.
    .arg("--config")
    .arg(r#"target.wasm32-unknown-unknown.rustflags=["-C", "link-arg=-zstack-size=8192"]"#)
    // Cargo passes its own RUSTFLAGS / encoded RUSTFLAGS to build.rs's
    // subprocesses, which take priority over our `--config` override. Clear
    // them so the override actually applies.
    .env_remove("CARGO_ENCODED_RUSTFLAGS")
    .env_remove("RUSTFLAGS");
    let output = cmd
        .output()
        .expect("failed to spawn cargo for wasm_builtins");
    if !output.status.success() {
        panic!(
            "wasm_builtins build failed (exit {})\n--- stdout ---\n{}\n--- stderr ---\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    let wasm_path = target_dir
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("wasm_builtins.wasm");
    let wasm = std::fs::read(&wasm_path)
        .unwrap_or_else(|e| panic!("missing {}: {}", wasm_path.display(), e));

    let parsed = parse_module(&wasm);
    // Emit this crate's `src/lib.rs` directly so the generated tables are
    // visible to humans and tooling under `crates/wasm_builtins_generated/src/`,
    // not buried under `target/.../OUT_DIR/`. Only rewrite when the content
    // actually changes - touching the file with identical content would
    // invalidate cargo's source fingerprint and force a rebuild loop on
    // every `cargo build`.
    let new_content = render_table(&parsed);
    let dest = manifest_dir.join("src").join("generated.rs");
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let needs_write = match std::fs::read_to_string(&dest) {
        Ok(existing) => existing != new_content,
        Err(_) => true,
    };
    if needs_write {
        std::fs::write(&dest, &new_content).expect("failed to emit src/generated.rs");
    }
}

/// A `Call $idx` site within a function body: where the LEB128 operand
/// lives (relative to the body bytes), how wide the original LEB was, and
/// which function it targets in the bundle. The runtime patcher rewrites
/// each operand to the function's freshly-allocated index in the output
/// module.
struct CallSite {
    operand_offset: u32,
    operand_width: u32,
    target: u32,
}

/// One entry per WASM function in the bundle.
struct FuncEntry {
    /// Stable name for exported functions; `None` for internal helpers.
    export_name: Option<String>,
    /// Code-section body bytes (locals-vec + instructions + END).
    body: Vec<u8>,
    /// Index into `BUILTIN_SIGS` for this function's signature.
    sig_idx: u32,
    /// All `Call` sites in body byte order.
    call_sites: Vec<CallSite>,
}

/// Imported function entry. The bundle's wasm-level function index
/// space puts imports first, defined functions after; bodies' `Call $idx`
/// operands referencing `idx < n_imports` target this table.
///
/// The codegen layer is responsible for satisfying each import — for the
/// only currently expected import, `__iec_raise`, it synthesizes an
/// inline `throw $rk_exception` helper and redirects calls to it.
struct Import {
    module: String,
    name: String,
    sig_idx: u32,
}

/// Extracted global declaration. Bodies reference these by index - we
/// preserve order so a `global.get 0` in a grafted body still hits the
/// same global once it's appended to the output module.
struct Global {
    val_type: ValType,
    mutable: bool,
    init_i32: i32,
}

/// Active data segment: contiguous bytes loaded into memory at a fixed
/// offset on instantiation. The bundle ships exactly one (polynomial
/// coefficient table); we keep the structure flexible in case that
/// changes.
struct DataSegment {
    memory_offset: u32,
    bytes: Vec<u8>,
}

struct ParsedModule {
    sigs: Vec<FuncType>,
    imports: Vec<Import>,
    funcs: Vec<FuncEntry>,
    globals: Vec<Global>,
    data: Vec<DataSegment>,
}

fn parse_module(wasm: &[u8]) -> ParsedModule {
    let mut sigs: Vec<FuncType> = Vec::new();
    let mut func_type_indices: Vec<u32> = Vec::new();
    let mut exports: Vec<(String, u32)> = Vec::new();
    let mut imports: Vec<Import> = Vec::new();
    let mut funcs: Vec<FuncEntry> = Vec::new();
    let mut globals: Vec<Global> = Vec::new();
    let mut data: Vec<DataSegment> = Vec::new();
    let mut code_idx: usize = 0;

    for payload in Parser::new(0).parse_all(wasm) {
        match payload.expect("malformed wasm") {
            Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    sigs.push(ty.expect("malformed type"));
                }
            }
            Payload::ImportSection(reader) => {
                use wasmparser::Imports;
                for group in reader {
                    let group = group.expect("malformed import group");
                    match group {
                        Imports::Single(_, imp) => {
                            if let TypeRef::Func(sig_idx) = imp.ty {
                                imports.push(Import {
                                    module: imp.module.to_string(),
                                    name: imp.name.to_string(),
                                    sig_idx,
                                });
                            }
                        }
                        Imports::Compact1 { module, items } => {
                            for item in items {
                                let item = item.expect("malformed compact1 item");
                                if let TypeRef::Func(sig_idx) = item.ty {
                                    imports.push(Import {
                                        module: module.to_string(),
                                        name: item.name.to_string(),
                                        sig_idx,
                                    });
                                }
                            }
                        }
                        Imports::Compact2 { module, ty, names } => {
                            if let TypeRef::Func(sig_idx) = ty {
                                for name in names {
                                    let name = name.expect("malformed compact2 name");
                                    imports.push(Import {
                                        module: module.to_string(),
                                        name: name.to_string(),
                                        sig_idx,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for entry in reader {
                    func_type_indices.push(entry.expect("malformed function entry"));
                }
            }
            Payload::GlobalSection(reader) => {
                for g in reader {
                    let g = g.expect("malformed global");
                    let init_i32 = const_expr_i32(&g.init_expr).unwrap_or_else(|| {
                        panic!("non-i32-const global init not supported in builtins")
                    });
                    globals.push(Global {
                        val_type: g.ty.content_type,
                        mutable: g.ty.mutable,
                        init_i32,
                    });
                }
            }
            Payload::DataSection(reader) => {
                for d in reader {
                    let d = d.expect("malformed data");
                    let offset = match &d.kind {
                        wasmparser::DataKind::Active { offset_expr, .. } => {
                            const_expr_i32(offset_expr)
                                .expect("active data offset must be i32.const")
                                as u32
                        }
                        wasmparser::DataKind::Passive => {
                            panic!("passive data segments not supported in builtins")
                        }
                    };
                    data.push(DataSegment {
                        memory_offset: offset,
                        bytes: d.data.to_vec(),
                    });
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.expect("malformed export");
                    if matches!(export.kind, wasmparser::ExternalKind::Func) {
                        exports.push((export.name.to_string(), export.index));
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                let range = body.range();
                let body_bytes = wasm[range.start..range.end].to_vec();
                let body_start = range.start;

                let mut call_sites = Vec::new();
                let mut ops = body.get_operators_reader().expect("op reader");
                while !ops.eof() {
                    let op_pos = ops.original_position();
                    let op = ops.read().expect("malformed op");
                    let next_pos = ops.original_position();
                    if let Operator::Call { function_index } = op {
                        // Call opcode is one byte; LEB128 operand follows.
                        let operand_start = op_pos + 1;
                        let operand_end = next_pos;
                        call_sites.push(CallSite {
                            operand_offset: (operand_start - body_start) as u32,
                            operand_width: (operand_end - operand_start) as u32,
                            target: function_index,
                        });
                    }
                }

                let sig_idx = func_type_indices
                    .get(code_idx)
                    .copied()
                    .expect("function section out of sync with code section");
                code_idx += 1;

                funcs.push(FuncEntry {
                    export_name: None,
                    body: body_bytes,
                    sig_idx,
                    call_sites,
                });
            }
            _ => {}
        }
    }

    // Stamp export names onto the matching function entries. Wasm export
    // indices count imports first; subtract `n_imports` to land in
    // `funcs` (which only holds defined functions, not imports).
    let n_imports = imports.len() as u32;
    for (name, idx) in exports {
        if idx < n_imports {
            // An export of an import is unusual but legal — skip rather
            // than misalign with `funcs`.
            continue;
        }
        let local_idx = (idx - n_imports) as usize;
        if let Some(entry) = funcs.get_mut(local_idx) {
            entry.export_name = Some(name);
        }
    }
    ParsedModule {
        sigs,
        imports,
        funcs,
        globals,
        data,
    }
}

/// Parse a `(i32.const N) end` constant initializer expression.
fn const_expr_i32(expr: &wasmparser::ConstExpr) -> Option<i32> {
    let mut reader = expr.get_operators_reader();
    let op = reader.read().ok()?;
    let _end = reader.read().ok()?;
    match op {
        Operator::I32Const { value } => Some(value),
        _ => None,
    }
}

fn vt_literal(vt: ValType) -> &'static str {
    match vt {
        ValType::I32 => "wasm_encoder::ValType::I32",
        ValType::I64 => "wasm_encoder::ValType::I64",
        ValType::F32 => "wasm_encoder::ValType::F32",
        ValType::F64 => "wasm_encoder::ValType::F64",
        other => panic!("unsupported val type in builtins: {other:?}"),
    }
}

fn render_table(parsed: &ParsedModule) -> String {
    let mut s = String::new();
    s.push_str("//! @generated by build.rs - do not edit\n");
    s.push_str("//!\n");
    s.push_str("//! Static tables extracted from the precompiled `wasm_builtins.wasm`.\n");
    s.push_str("//! Consumed by `wasm_codegen` to graft math intrinsics into IEC outputs.\n\n");

    // Function signatures, indexed by sig_idx.
    s.push_str("pub struct BuiltinSig {\n");
    s.push_str("    pub params: &'static [wasm_encoder::ValType],\n");
    s.push_str("    pub results: &'static [wasm_encoder::ValType],\n");
    s.push_str("}\n\n");
    s.push_str("pub static BUILTIN_SIGS: &[BuiltinSig] = &[\n");
    for sig in &parsed.sigs {
        let params: Vec<&str> = sig.params().iter().copied().map(vt_literal).collect();
        let results: Vec<&str> = sig.results().iter().copied().map(vt_literal).collect();
        s.push_str("    BuiltinSig {\n");
        s.push_str(&format!("        params: &[{}],\n", params.join(", ")));
        s.push_str(&format!("        results: &[{}],\n", results.join(", ")));
        s.push_str("    },\n");
    }
    s.push_str("];\n\n");

    // Imports. Function imports occupy wasm function indices `0..n_imports`;
    // bodies whose `Call $idx` operand falls in that range target this
    // table, and the codegen is responsible for satisfying each one
    // (typically by synthesizing a helper at graft time).
    s.push_str("pub struct BuiltinImport {\n");
    s.push_str("    pub module: &'static str,\n");
    s.push_str("    pub name: &'static str,\n");
    s.push_str("    pub sig_idx: u32,\n");
    s.push_str("}\n\n");
    s.push_str("pub static BUILTIN_IMPORTS: &[BuiltinImport] = &[\n");
    for imp in &parsed.imports {
        s.push_str("    BuiltinImport {\n");
        s.push_str(&format!("        module: {:?},\n", imp.module));
        s.push_str(&format!("        name: {:?},\n", imp.name));
        s.push_str(&format!("        sig_idx: {},\n", imp.sig_idx));
        s.push_str("    },\n");
    }
    s.push_str("];\n\n");
    s.push_str(&format!(
        "/// Number of imported functions in the bundle's wasm function\n\
         /// index space. Targets in `BuiltinCallSite::target` below this\n\
         /// value refer to `BUILTIN_IMPORTS`; targets at or above index\n\
         /// into `BUILTIN_FUNCS` after subtracting `N_IMPORTS`.\n\
         pub const N_IMPORTS: u32 = {};\n\n",
        parsed.imports.len()
    ));

    // Per-function metadata.
    s.push_str("pub struct BuiltinCallSite {\n");
    s.push_str("    pub operand_offset: u32,\n");
    s.push_str("    pub operand_width: u32,\n");
    s.push_str("    /// Raw wasm function index from the bundle. Compare\n");
    s.push_str("    /// against `N_IMPORTS` to distinguish imports vs. locals.\n");
    s.push_str("    pub target: u32,\n");
    s.push_str("}\n\n");
    s.push_str("pub struct BuiltinFunc {\n");
    s.push_str("    pub body: &'static [u8],\n");
    s.push_str("    pub sig_idx: u32,\n");
    s.push_str("    pub call_sites: &'static [BuiltinCallSite],\n");
    s.push_str("}\n\n");

    s.push_str("pub static BUILTIN_FUNCS: &[BuiltinFunc] = &[\n");
    for f in &parsed.funcs {
        s.push_str("    BuiltinFunc {\n");
        s.push_str(&format!("        body: &{:?},\n", f.body));
        s.push_str(&format!("        sig_idx: {},\n", f.sig_idx));
        s.push_str("        call_sites: &[\n");
        for cs in &f.call_sites {
            s.push_str(&format!(
                "            BuiltinCallSite {{ operand_offset: {}, operand_width: {}, target: {} }},\n",
                cs.operand_offset, cs.operand_width, cs.target
            ));
        }
        s.push_str("        ],\n");
        s.push_str("    },\n");
    }
    s.push_str("];\n\n");

    // phf::Map<&'static str, u32> - export name → function index in BUILTIN_FUNCS.
    s.push_str("pub static BUILTIN_NAMES: phf::Map<&'static str, u32> = phf::phf_map! {\n");
    for (i, f) in parsed.funcs.iter().enumerate() {
        if let Some(name) = &f.export_name {
            // Convention: `f32_sin` (export) ↔ `f32.sin` (codegen lookup).
            let dotted = name.replacen('_', ".", 1);
            s.push_str(&format!("    \"{}\" => {},\n", dotted, i));
        }
    }
    s.push_str("};\n\n");

    // Globals: (val_type, mutable, init_i32). Index 0 is the libm shadow-stack
    // pointer; subsequent entries are static markers (`__data_end`,
    // `__heap_base`) that bodies may also reference.
    s.push_str("pub struct BuiltinGlobal {\n");
    s.push_str("    pub val_type: wasm_encoder::ValType,\n");
    s.push_str("    pub mutable: bool,\n");
    s.push_str("    pub init_i32: i32,\n");
    s.push_str("}\n\n");
    s.push_str("pub static BUILTIN_GLOBALS: &[BuiltinGlobal] = &[\n");
    for g in &parsed.globals {
        s.push_str("    BuiltinGlobal {\n");
        s.push_str(&format!("        val_type: {},\n", vt_literal(g.val_type)));
        s.push_str(&format!("        mutable: {},\n", g.mutable));
        s.push_str(&format!("        init_i32: {},\n", g.init_i32));
        s.push_str("    },\n");
    }
    s.push_str("];\n\n");

    // Data segments: active loads at fixed memory offsets. The bundle's
    // memory addresses are preserved as-is - output binaries reserve the
    // span via WasmGen and the IEC layout starts above it.
    s.push_str("pub struct BuiltinData {\n");
    s.push_str("    pub memory_offset: u32,\n");
    s.push_str("    pub bytes: &'static [u8],\n");
    s.push_str("}\n\n");
    s.push_str("pub static BUILTIN_DATA: &[BuiltinData] = &[\n");
    for d in &parsed.data {
        s.push_str("    BuiltinData {\n");
        s.push_str(&format!("        memory_offset: {},\n", d.memory_offset));
        s.push_str(&format!("        bytes: &{:?},\n", d.bytes));
        s.push_str("    },\n");
    }
    s.push_str("];\n\n");

    // Total memory bytes the bundle expects to own (data segments end +
    // any post-data padding). The IEC layout adds its own statics on top.
    let bundle_top = parsed
        .data
        .iter()
        .map(|d| d.memory_offset as usize + d.bytes.len())
        .max()
        .unwrap_or(0);
    s.push_str(&format!(
        "/// Highest memory address the bundle's data section occupies.\n\
         /// IEC static layout must start at or above this offset.\n\
         pub const BUNDLE_MEMORY_TOP: u32 = {};\n",
        bundle_top
    ));
    s
}
