// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! rk-bindgen: the ST declarations of the interfaces a WIT world imports,
//! as wit-bindgen writes Rust for them. One module per family of WIT
//! types. Each case generates a small WIT package's declarations and shows
//! them; rk must take them, and every import of the module it builds from
//! them must have the core type the canonical ABI gives the WIT function
//! ([`abi_mismatches`]). `wasi` generates every interface of the WASI rk
//! carries and builds them into a component, which checks each import
//! against WASI's WIT; `runtime` runs generated declarations on wasmtime.

mod aliases;
mod async_calls;
mod built_ins;
mod enums;
mod names;
mod options_and_results;
mod records;
mod resources;
mod runtime;
mod scalars;
mod skips;
mod wasi;

use db::RootDatabase;
use wit_parser::Resolve;
use wit_parser::abi::{AbiVariant, WasmType};

use crate::tests::utils::{lower_workspace, test_diagnostics};

/// What rk-bindgen writes for a world, with the WIT it read.
pub(super) struct Bindings {
    pub(super) resolve: Resolve,
    pub(super) files: Vec<rk_bindgen::File>,
}

/// The files rk-bindgen writes for the world in `wit`, a package of its own
/// beside the WASI rk carries.
pub(super) fn generated(wit: &str) -> Bindings {
    let (resolve, world) = rk_bindgen::resolve(wit).expect("the WIT resolves");
    let files = rk_bindgen::generate(&resolve, world).expect("it generates");
    Bindings { resolve, files }
}

/// Each file after a line with its name, without the comment every file
/// starts with.
pub(super) fn shown(files: &[rk_bindgen::File]) -> String {
    files
        .iter()
        .map(|file| {
            let text = match file.text.split_once("DO NOT EDIT! */\n") {
                Some((_, rest)) => rest,
                None => &file.text,
            };
            format!("// {}\n{text}", file.name)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The diagnostics rk reports on `files` and `extra`: none when it takes
/// them, and then the workspace also lowers and validates.
pub(super) fn reported(
    db: &mut RootDatabase,
    files: &[rk_bindgen::File],
    extra: &[&str],
) -> String {
    let mut texts: Vec<&str> = files.iter().map(|file| file.text.as_str()).collect();
    texts.extend(extra);
    test_diagnostics(db, &texts)
}

/// The MIR and the module rk builds from `files` and `extra`, which it must
/// take.
pub(super) fn built(
    db: &mut RootDatabase,
    files: &[rk_bindgen::File],
    extra: &[&str],
) -> (mir::MirModule, Vec<u8>) {
    assert_eq!(
        reported(db, files, extra),
        "",
        "rk takes the generated code"
    );
    let module = lower_workspace(db);
    let core = wasm_codegen::generate_wasm(db, &module).finish();
    (module, core)
}

/// The module rk builds from `files` and `extra`, which it must take.
pub(super) fn core(db: &mut RootDatabase, files: &[rk_bindgen::File], extra: &[&str]) -> Vec<u8> {
    built(db, files, extra).1
}

/// The component rk builds from `mir` and `core`, as `rk compile` wraps a
/// module: the build checks each WASI import against WASI's WIT.
pub(super) fn wrapped(
    db: &RootDatabase,
    mir: &mir::MirModule,
    core: &[u8],
) -> rk::component::Component {
    let interfaces = rk::component::interfaces(db, mir).expect("typed interfaces");
    rk::component::wrap(core, "plant", &interfaces).expect("the module wraps")
}

/// What each file declares, by name, and what it leaves out, and why.
pub(super) fn summary(files: &[rk_bindgen::File]) -> String {
    files
        .iter()
        .map(|file| {
            let pous: Vec<&str> = file
                .text
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    line.strip_prefix("FUNCTION_BLOCK ")
                        .or_else(|| line.strip_prefix("FUNCTION "))
                        .or_else(|| line.strip_prefix("TYPE "))
                        .and_then(|rest| rest.split([' ', ':']).next())
                })
                .collect();
            let mut text = format!("{}: {}", file.name, pous.join(", "));
            for skipped in &file.skipped {
                text.push_str(&format!(
                    "\n    left out {}: {}",
                    skipped.item, skipped.reason
                ));
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The ST rk-bindgen writes for `wit`, once rk has taken it and every
/// import of its module has the core type the canonical ABI gives it.
pub(super) fn checked(db: &mut RootDatabase, wit: &str) -> String {
    let bindings = generated(wit);
    let module = core(db, &bindings.files, &[]);
    assert_abi(&bindings, &module);
    shown(&bindings.files)
}

/// Checks that each import of `module` from an interface of the WIT has the
/// core type the canonical ABI gives it, and that every declaration was
/// checked: one import for each `{extern` the files hold.
pub(super) fn assert_abi(bindings: &Bindings, module: &[u8]) {
    let (checked, mismatches) = abi_mismatches(&bindings.resolve, module);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
    let declared: usize = bindings
        .files
        .iter()
        .map(|file| file.text.matches("{extern ").count())
        .sum();
    assert_eq!(checked, declared, "each declaration is one import");
}

/// How many imports of `core` come from an interface of `resolve`, and
/// those whose core type is not the canonical ABI's: a function's as
/// `Resolve::wasm_signature` gives it, lowered async after `[async-lower]`,
/// and a built-in's as wit-component checks it.
pub(super) fn abi_mismatches(resolve: &Resolve, core: &[u8]) -> (usize, Vec<String>) {
    let mut types = Vec::new();
    let mut checked = 0;
    let mut mismatches = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(core) {
        match payload.expect("a valid module") {
            wasmparser::Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    types.push(ty.expect("a function type"));
                }
            }
            wasmparser::Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.expect("an import");
                    let wasmparser::TypeRef::Func(index) = import.ty else {
                        continue;
                    };
                    let expected = match expected_type(resolve, import.module, import.name) {
                        Ok(Some(expected)) => expected,
                        Ok(None) => continue,
                        Err(why) => {
                            mismatches.push(format!("{} {}: {why}", import.module, import.name));
                            continue;
                        }
                    };
                    checked += 1;
                    let actual: &wasmparser::FuncType = &types[index as usize];
                    if actual.params() != expected.0.as_slice()
                        || actual.results() != expected.1.as_slice()
                    {
                        mismatches.push(format!(
                            "{} {}: {:?} -> {:?}, where the canonical ABI has {:?} -> {:?}",
                            import.module,
                            import.name,
                            actual.params(),
                            actual.results(),
                            expected.0,
                            expected.1
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    (checked, mismatches)
}

type CoreType = (Vec<wasmparser::ValType>, Vec<wasmparser::ValType>);

/// The core type of `name` imported from `module`: `None` when `module` is
/// no interface of `resolve`, `Err` when the interface has no such name.
fn expected_type(resolve: &Resolve, module: &str, name: &str) -> Result<Option<CoreType>, String> {
    use wasmparser::ValType::{F32, F64, I32, I64};
    let Some((interface, _)) = resolve
        .interfaces
        .iter()
        .find(|(id, _)| resolve.id_of(*id).as_deref() == Some(module))
    else {
        return Ok(None);
    };
    let (asynchronous, name) = match name.strip_prefix("[async-lower]") {
        Some(rest) => (true, rest),
        None => (false, name),
    };
    if let Some((built_in, _)) = hir::hir_def::pous::pragma::builtin_name(name) {
        return Ok(Some(match built_in {
            "stream-new" | "future-new" => (vec![], vec![I64]),
            "stream-read" | "stream-write" => (vec![I32; 3], vec![I32]),
            "future-read" | "future-write" => (vec![I32; 2], vec![I32]),
            _ => (vec![I32], vec![]),
        }));
    }
    if name.starts_with("[resource-drop]") {
        return Ok(Some((vec![I32], vec![])));
    }
    let function = resolve.interfaces[interface]
        .functions
        .get(name)
        .ok_or("no such function in the interface")?;
    let variant = match asynchronous {
        true => AbiVariant::GuestImportAsync,
        false => AbiVariant::GuestImport,
    };
    let signature = resolve.wasm_signature(variant, function);
    let core = |types: &[WasmType]| {
        types
            .iter()
            .map(|ty| match ty {
                WasmType::I32 | WasmType::Pointer | WasmType::Length => I32,
                WasmType::I64 | WasmType::PointerOrI64 => I64,
                WasmType::F32 => F32,
                WasmType::F64 => F64,
            })
            .collect()
    };
    Ok(Some((core(&signature.params), core(&signature.results))))
}
