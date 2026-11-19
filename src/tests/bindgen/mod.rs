// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! rk-bindgen: the ST declarations of the interfaces a WIT world imports,
//! as wit-bindgen writes Rust for them. One module per family of WIT
//! types. Each case generates a small WIT package's declarations and shows
//! them; rk must take them, and every import of the module it builds from
//! them must have the core type the canonical ABI gives the WIT function
//! ([`abi_mismatches`]), and take at least the bytes the host writes where
//! a result comes back in memory ([`short_results`]). `wasi` generates
//! every interface of WASI's packages, as WASI's repository writes them,
//! and builds them into a component, which checks each import against
//! WASI's WIT, and Std's from Std's own WIT folder;
//! `runtime` runs generated declarations on wasmtime;
//! `packages` generates for a package of any namespace, from a WIT folder.
//! What a result read out of words holds is checked there too, against a
//! host of the test's own ([`hosted`]): wasmtime lays out what the host
//! returns, and the generated code reads it back; and what the generated
//! code packs for the host, wasmtime reads.

mod aliases;
mod async_calls;
mod built_ins;
mod components;
mod enums;
mod flags;
mod lists;
mod names;
mod options_and_results;
mod packages;
mod records;
mod resources;
mod runtime;
mod scalars;
mod skips;
mod strings;
mod variants;
mod wasi;

use db::RootDatabase;
use wit_parser::Resolve;
use wit_parser::abi::{AbiVariant, WasmType};

use crate::tests::cli::component_host::{Host, Plc};
use crate::tests::utils::{lower_workspace, stdlib_wit, test_diagnostics, wasi_packages};

/// What rk-bindgen writes for a world, with the WIT it read.
pub(super) struct Bindings {
    pub(super) resolve: Resolve,
    pub(super) files: Vec<rk_bindgen::File>,
}

/// The world in `wit`, a package of its own beside WASI's packages, as
/// WASI's repository writes them, whose interfaces it may import.
pub(super) fn resolved(wit: &str) -> Result<(Resolve, wit_parser::WorldId), String> {
    rk_bindgen::resolve(wit, &wasi_packages())
}

/// The files rk-bindgen writes for the world in `wit`, a package of its own
/// beside WASI's packages.
pub(super) fn generated(wit: &str) -> Bindings {
    let (resolve, world) = resolved(wit).expect("the WIT resolves");
    let files = rk_bindgen::generate(&resolve, world).expect("it generates");
    Bindings { resolve, files }
}

/// The files rk-bindgen writes for Std's WIT folder: Std's declarations.
pub(super) fn generated_std() -> Bindings {
    let (resolve, world) =
        rk_bindgen::resolve_folder(&stdlib_wit(), None).expect("Std's WIT resolves");
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

/// The conversions the code that reads a result out of words or packs a
/// parameter into them calls, each cut out of `Std.Convert` as the standard
/// library writes it: the whole file takes a test half a minute to build.
pub(super) fn std_convert() -> String {
    let convert = include_str!("../../../stdlib/Convert.st");
    let mut text = String::from("NAMESPACE Std.Convert\n");
    for name in [
        "BYTE_TO_USINT",
        "WORD_TO_UINT",
        "DWORD_TO_UDINT",
        "LWORD_TO_ULINT",
        "BYTE_TO_SINT",
        "WORD_TO_INT",
        "DWORD_TO_DINT",
        "LWORD_TO_LINT",
        "DWORD_TO_REAL",
        "LWORD_TO_LREAL",
        "USINT_TO_BYTE",
        "UINT_TO_WORD",
        "UDINT_TO_DWORD",
        "ULINT_TO_LWORD",
        "SINT_TO_BYTE",
        "INT_TO_WORD",
        "DINT_TO_DWORD",
        "LINT_TO_LWORD",
        "REAL_TO_DWORD",
        "LREAL_TO_LWORD",
        "DINT_TO_UDINT",
    ] {
        let start = convert
            .find(&format!("\tFUNCTION {name}:"))
            .unwrap_or_else(|| panic!("`Std.Convert` has `{name}`"));
        let end = convert[start..]
            .find("END_FUNCTION")
            .expect("the function ends");
        text.push_str(&convert[start..start + end]);
        text.push_str("END_FUNCTION\n");
    }
    text.push_str("END_NAMESPACE\n");
    text
}

/// The bounds of an array, `LOWER_BOUND` and `UPPER_BOUND`, cut out of
/// `Std.Arrays` as the standard library writes them, which the code that
/// passes a list from the caller's array checks its count with.
pub(super) fn std_arrays() -> String {
    let arrays = include_str!("../../../stdlib/Arrays.st");
    let mut text = String::from("NAMESPACE Std.Arrays\n");
    for name in ["LOWER_BOUND", "UPPER_BOUND"] {
        let start = arrays
            .find(&format!("\tFUNCTION {name}:"))
            .unwrap_or_else(|| panic!("`Std.Arrays` has `{name}`"));
        let end = arrays[start..]
            .find("END_FUNCTION")
            .expect("the function ends");
        text.push_str(&arrays[start..start + end]);
        text.push_str("END_FUNCTION\n");
    }
    text.push_str("END_NAMESPACE\n");
    text
}

/// The diagnostics rk reports on `files` and `extra`: none when it takes
/// them, and then the workspace also lowers and validates. Code that
/// converts what it reads is taken with `Std.Convert` as its library, and
/// code that passes a list with the bounds of `Std.Arrays`.
pub(super) fn reported(
    db: &mut RootDatabase,
    files: &[rk_bindgen::File],
    extra: &[&str],
) -> String {
    let mut texts: Vec<&str> = files.iter().map(|file| file.text.as_str()).collect();
    texts.extend(extra);
    let mut libraries = Vec::new();
    if texts.iter().any(|text| text.contains("USING Std.Convert;")) {
        libraries.push(std_convert());
    }
    if texts.iter().any(|text| text.contains("Std.Arrays.")) {
        libraries.push(std_arrays());
    }
    if !libraries.is_empty() {
        let libraries: Vec<&str> = libraries.iter().map(String::as_str).collect();
        crate::tests::utils::add_library_sources(db, &libraries);
    }
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

/// The component rk builds from `mir` and `core`, as `rk compile` wraps a
/// module whose WIT the folders `wit` hold, beside those the database's
/// libraries hold: the build checks each WASI import against it.
pub(super) fn wrapped_with(
    db: &RootDatabase,
    mir: &mir::MirModule,
    core: &[u8],
    wit: &[std::path::PathBuf],
) -> rk::component::Component {
    let mut interfaces = rk::component::interfaces(db, mir).expect("typed interfaces");
    interfaces.wit.extend(wit.iter().cloned());
    rk::component::wrap(core, "plant", &interfaces).expect("the module wraps")
}

/// [`wrapped_with`] WASI's packages, as WASI's repository writes them: what
/// a library whose declarations are generated from them ships.
pub(super) fn wrapped(
    db: &RootDatabase,
    mir: &mir::MirModule,
    core: &[u8],
) -> rk::component::Component {
    wrapped_with(db, mir, core, &wasi_packages())
}

/// A component on a wasmtime host, built from the declarations generated
/// for `wit`, a package of the test's own, from `program` and from a task
/// that runs its PROGRAM `P`, whose verdict is the output a scan gives
/// back. `define` gives the host the package's functions: what they return
/// is laid out by wasmtime, as the canonical ABI says, and read back by the
/// generated code.
pub(super) fn hosted(
    db: &mut RootDatabase,
    wit: &str,
    program: &str,
    define: impl FnOnce(&mut wasmtime::component::Linker<Host>),
) -> Plc {
    let configuration = r#"
        CONFIGURATION Cfg
        VAR_GLOBAL output AT %QD0 : DINT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Main WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let bindings = generated(wit);
    let (mir, core) = built(db, &bindings.files, &[program, configuration]);
    // A program may declare an import of the package by hand too.
    let by_hand = program.matches("{extern ").count();
    assert_abi_with(&bindings, &mir, &core, by_hand);
    Plc::load_with(
        &component_of(wit, &core),
        wasmtime_wasi::WasiCtxBuilder::new().build(),
        define,
    )
}

/// The component of `core`, a module that imports from `wit`, a package
/// of the test's own: its world imports each interface the module imports
/// from and exports the control interface, as `rk compile` types a module
/// whose WIT a library ships.
fn component_of(wit: &str, core: &[u8]) -> Vec<u8> {
    let parse = |dir: &str, files: &[(&str, &str)]| {
        let mut map = wit_parser::SourceMap::new();
        for (file, text) in files {
            map.push_str(&format!("{dir}/{file}"), text.to_string());
        }
        map.parse()
            .unwrap_or_else(|(_, e)| panic!("the WIT in `{dir}` parses: {e}"))
    };
    let mut imports = std::collections::BTreeSet::new();
    for payload in wasmparser::Parser::new(0).parse_all(core) {
        if let wasmparser::Payload::ImportSection(reader) = payload.expect("a valid module") {
            for import in reader.into_imports() {
                let module = import.expect("an import").module;
                if module.contains('/') {
                    imports.insert(module.to_string());
                }
            }
        }
    }
    let imports: String = imports
        .iter()
        .map(|interface| format!("    import {interface};\n"))
        .collect();
    let world = format!(
        "package rk:hosted;\n\nworld plc {{\n{imports}    export {};\n}}\n",
        debug_format::wit::CONTROL_INTERFACE
    );
    let mut resolve = Resolve {
        all_features: true,
        ..Default::default()
    };
    let mut deps: Vec<_> = debug_format::wit::packages()
        .map(|package| parse(package.dir, package.files))
        .collect();
    // WASI's packages, each its folder's `.wit` files.
    for folder in wasi_packages() {
        let mut map = wit_parser::SourceMap::new();
        map.push_dir(&folder).expect("a package's files");
        deps.push(
            map.parse()
                .unwrap_or_else(|(_, e)| panic!("{}: {e}", folder.display())),
        );
    }
    deps.push(parse("example", &[("example.wit", wit)]));
    let package = resolve
        .push_groups(parse(".", &[("world.wit", &world)]), deps)
        .expect("the component's WIT resolves");
    let world = resolve
        .select_world(&[package], Some("plc"))
        .expect("the component's world");
    let mut module = core.to_vec();
    wit_component::embed_component_metadata(
        &mut module,
        &resolve,
        world,
        wit_component::StringEncoding::UTF8,
        false,
    )
    .expect("the module is typed with its world");
    wit_component::ComponentEncoder::default()
        .validate(true)
        .module(&module)
        .and_then(|encoder| encoder.encode())
        .expect("the module wraps into a component")
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
                        .or_else(|| line.strip_prefix("FUNCTION PRIVATE "))
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
    let (mir, module) = built(db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
    shown(&bindings.files)
}

/// Checks that each import of `module` from an interface of the WIT has the
/// core type the canonical ABI gives it, that every declaration was
/// checked, one import for each `{extern` the files hold, and that each
/// result the host writes in memory fits what `mir` declares for it.
pub(super) fn assert_abi(bindings: &Bindings, mir: &mir::MirModule, module: &[u8]) {
    assert_abi_with(bindings, mir, module, 0);
}

/// [`assert_abi`], `by_hand` declarations of the package's imports written
/// beside the generated ones.
fn assert_abi_with(bindings: &Bindings, mir: &mir::MirModule, module: &[u8], by_hand: usize) {
    let short = short_results(&bindings.resolve, mir);
    assert!(short.is_empty(), "{short:#?}");
    let (checked, mismatches) = abi_mismatches(&bindings.resolve, module);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
    let declared: usize = bindings
        .files
        .iter()
        .map(|file| file.text.matches("{extern ").count())
        .sum();
    assert_eq!(
        checked,
        declared + by_hand,
        "each declaration is one import"
    );
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

/// The imports of `mir` whose result the host writes in memory, a call's
/// or a future's value, and whose declaration takes fewer bytes than the
/// canonical ABI lays the result out in: the host would write past what rk
/// keeps for it. The outputs are laid out as a record is.
pub(super) fn short_results(resolve: &Resolve, mir: &mir::MirModule) -> Vec<String> {
    let mut sizes = wit_parser::SizeAlign::default();
    sizes.fill(resolve).expect("the types are laid out");
    let function = |module: &str, name: &str| {
        let (_, interface) = resolve
            .interfaces
            .iter()
            .find(|(id, _)| resolve.id_of(*id).as_deref() == Some(module))?;
        interface.functions.get(name)
    };
    let declared = |types: &mut dyn Iterator<Item = &mir::types::MirType>| {
        let (mut size, mut align) = (0u32, 1u32);
        for ty in types {
            size = size.next_multiple_of(ty.alignment()) + ty.size_bytes();
            align = align.max(ty.alignment());
        }
        size.next_multiple_of(align) as usize
    };
    let mut short = Vec::new();
    let mut check =
        |module: &str, name: &str, written: Option<wit_parser::Type>, declared: usize| {
            let Some(written) = written else { return };
            let written = sizes.size(&written).size_wasm32();
            if declared < written {
                short.push(format!(
                    "{module} {name}: {declared} bytes declared, where the host writes {written}"
                ));
            }
        };
    for import in &mir.extern_functions {
        let (module, name) = (import.module.as_str(), import.import_name.as_str());
        // An async import's result is its block's, below.
        if import.asynchronous || hir::hir_def::pous::pragma::builtin_name(name).is_some() {
            continue;
        }
        let Some(function) = function(module, name) else {
            continue;
        };
        if resolve
            .wasm_signature(AbiVariant::GuestImport, function)
            .retptr
        {
            check(
                module,
                name,
                function.result,
                declared(&mut import.results()),
            );
        }
    }
    for call in mir.async_calls.values() {
        let Some(import) = mir
            .extern_functions
            .iter()
            .find(|import| import.name == call.import)
        else {
            continue;
        };
        let module = import.module.as_str();
        let name = import
            .import_name
            .strip_prefix("[async-lower]")
            .unwrap_or(&import.import_name);
        let declared = declared(&mut call.results.iter().map(|(_, ty)| ty));
        match hir::hir_def::pous::pragma::builtin_name(name) {
            None => {
                if let Some(function) = function(module, name) {
                    check(module, name, function.result, declared);
                }
            }
            // `[future-read-N]f`: the value of the future numbered N among
            // the streams and futures of `f`'s type.
            Some(("future-read", of)) => {
                let number = name[1..]
                    .split_once(']')
                    .and_then(|(inside, _)| inside.rsplit_once('-'))
                    .and_then(|(_, number)| number.parse::<usize>().ok());
                let value = function(module, of)
                    .zip(number)
                    .and_then(|(function, number)| {
                        function
                            .find_futures_and_streams(resolve)
                            .get(number)
                            .copied()
                    })
                    .and_then(|id| match resolve.types[id].kind {
                        wit_parser::TypeDefKind::Future(value) => value,
                        _ => None,
                    });
                check(module, name, value, declared);
            }
            Some(_) => {}
        }
    }
    short
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
