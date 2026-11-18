// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A built module wrapped into a component, typed by a world generated at
//! each build.
//!
//! The world imports the interfaces of packages the module calls, as the
//! WIT folders of the libraries and of the workspace declare them (the
//! standard library's holds the WASI interfaces its blocks call), the
//! interfaces of plain modules, typed from the `{extern}` FUNCTIONs that
//! import them, and exports the control interface every module has
//! (`rk:plc/control`) and the workspace's `{export}` FUNCTIONs, in an
//! interface named after the project.
//! The component carries the types of what it imports and exports, so rk
//! writes no WIT beside it: a host's tools read them from the binary
//! (`wasm-tools component wit`).
//! Binaryen reads core modules only, so the wrapping is the build's last
//! step, after `wasm-opt`.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use debug_format::wit;
pub use hir::hir_def::pous::pragma::wit_name;
use hir::hir_def::pous::pragma::{ImportModule, import_module, names_a_package};
use hir::hir_ty::exports::wit_key;
use mir::function::{MirExternFunction, MirParamKind};
use mir::types::MirType;

/// The world every component exports its control interface from.
pub const WORLD: &str = "plc";

/// A component, and the text of the world that types it.
pub struct Component {
    pub bytes: Vec<u8>,
    pub world: String,
}

/// A function the module imports from a plain module, typed in WIT from
/// the `{extern}` FUNCTION that declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFunction {
    /// The FUNCTION, as the module names it.
    pub declared_by: String,
    /// `modbus`.
    pub module: String,
    /// `read-coils`.
    pub name: String,
    /// Each parameter's WIT name and type.
    pub params: Vec<(String, &'static str)>,
    /// Each VAR_OUTPUT's WIT name and type. With any, the function returns
    /// the record of them and its return value ([`record_name`]).
    pub outputs: Vec<(String, &'static str)>,
    pub result: Option<&'static str>,
    /// Whether an `{extern}` FUNCTION_BLOCK imports it, async: an `async
    /// func` in WIT.
    pub asynchronous: bool,
}

/// A FUNCTION the component exports for a host to call, typed in WIT from
/// its `{export}` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedFunction {
    /// `scale`.
    pub name: String,
    /// Each parameter's WIT name and type.
    pub params: Vec<(String, &'static str)>,
    /// Each VAR_OUTPUT's WIT name and type. With any, the function returns
    /// the record of them and its return value ([`record_name`]).
    pub outputs: Vec<(String, &'static str)>,
    /// What it returns when nothing stops it: the function's result is a
    /// `result` of it, or the message of what did.
    pub result: Option<&'static str>,
}

/// What a module exchanges with its host besides the interfaces of
/// packages and the control interface, typed in WIT from its declarations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Interfaces {
    /// The functions it imports from plain modules.
    pub imports: Vec<HostFunction>,
    /// The interface its `{export}` FUNCTIONs are exported in: the
    /// project's name, as the module's own exports spell it.
    pub export_interface: String,
    /// Its `{export}` FUNCTIONs.
    pub exports: Vec<ExportedFunction>,
    /// The POU that declares each import, by the import's module and its
    /// name as the module imports it: what a build error about it names.
    pub declared: BTreeMap<(String, String), String>,
    /// What each stream's read or write moves, by its import as
    /// [`Interfaces::declared`] keys it: what its block's buffer holds.
    pub moved: BTreeMap<(String, String), Moved>,
    /// How many bytes each import's declaration holds of its result, by
    /// its import as [`Interfaces::declared`] keys it: the record of the
    /// fields it reads back, a call's outputs, or a future read's value.
    pub held: BTreeMap<(String, String), u32>,
    /// The WIT folders of the workspace and of each library it reads,
    /// `wit/` at their root, whether there is one or not: the packages they
    /// hold type the imports of their interfaces, WASI's as the standard
    /// library's folder holds them, and no other import names a package.
    pub wit: Vec<std::path::PathBuf>,
}

/// What a stream's read or write moves, by the buffer its block declares
/// (E1516): the build checks that the stream carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// Bytes, from a STRING or into a STRING or an ARRAY OF BYTE.
    Bytes,
    /// A resource's handles, into an ARRAY OF UDINT.
    Handles,
}

/// The interfaces of `module`, typed in WIT: what HIR accepts on an
/// `{extern}` from a plain module, and on an `{export}`, has a WIT type and
/// name.
pub fn interfaces(
    db: &dyn db::WorkspaceDataBase,
    module: &mir::MirModule,
) -> Result<Interfaces, String> {
    let mut exports = Vec::new();
    for func in &module.functions {
        let Some(name) = &func.wit_export else {
            continue;
        };
        let untyped = |what: &str| {
            format!(
                "internal compiler error: {what} of `{}` has no WIT type",
                func.name.text(db)
            )
        };
        let (mut params, mut outputs) = (Vec::new(), Vec::new());
        for param in &func.params {
            let declared = param.declared_name(db);
            let wit_name = hir::hir_def::pous::pragma::wit_parameter_name(declared)
                .ok_or_else(|| untyped(&format!("the name of `{declared}`")))?;
            // An output is passed as the address it is written at.
            let (held, list) = match (&param.kind, &param.ty) {
                (MirParamKind::Output, MirType::Pointer(held)) => (held.as_ref(), &mut outputs),
                (_, ty) => (ty, &mut params),
            };
            let ty = wit_type(held).ok_or_else(|| untyped(&format!("`{declared}`")))?;
            list.push((wit_name, ty));
        }
        let result = match &func.return_type {
            Some(ty) => Some(wit_type(ty).ok_or_else(|| untyped("the return type"))?),
            None => None,
        };
        exports.push(ExportedFunction {
            name: name.to_string(),
            params,
            outputs,
            result,
        });
    }
    // An async import's function is its block's, `Block$import`.
    let declared = module
        .extern_functions
        .iter()
        .map(|ext| {
            let name = ext.name.text(db);
            (
                (ext.module.to_string(), ext.import_name.to_string()),
                name.strip_suffix("$import").unwrap_or(name).to_string(),
            )
        })
        .collect();
    Ok(Interfaces {
        imports: host_functions(db, &module.extern_functions)?,
        export_interface: hir::hir_ty::exports::export_interface(db),
        exports,
        declared,
        moved: moved(module),
        held: held(module),
        wit: wit_folders(db),
    })
}

/// The `wit/` folders of the libraries the workspace reads, the standard
/// library's first, then the workspace's own, whether there is one or not.
fn wit_folders(db: &dyn db::WorkspaceDataBase) -> Vec<std::path::PathBuf> {
    let Some(workspace) = db::workspace::Workspace::try_get(db) else {
        return Vec::new();
    };
    workspace
        .libraries(db)
        .iter()
        .map(|library| library.dir.clone())
        .chain(workspace.workspace_folder(db).cloned())
        .map(|root| root.join("wit"))
        .collect()
}

/// A package's `.wit` files by name: each one's text, and the folder it is
/// read from.
type Files = BTreeMap<String, (String, std::path::PathBuf)>;

/// A WIT package a folder ships: its name and version as its files declare
/// them, and its files.
struct Shipped {
    id: String,
    files: Files,
    group: wit_parser::UnresolvedPackageGroup,
}

impl Shipped {
    /// The package `files` write, parsed as one package in the order of
    /// their names, each named by its folder where the parser reports.
    fn parsed(files: Files) -> Result<Self, String> {
        let mut map = wit_parser::SourceMap::new();
        for (name, (text, from)) in &files {
            map.push_str(&from.join(name).display().to_string(), text.clone());
        }
        let folders: BTreeSet<String> = files
            .values()
            .map(|(_, from)| format!("`{}`", from.display()))
            .collect();
        let group = map.parse().map_err(|(_, e)| {
            let folders: Vec<String> = folders.into_iter().collect();
            format!("parsing the WIT in {}: {e}", folders.join(" and "))
        })?;
        Ok(Shipped {
            id: group.main.name.to_string(),
            files,
            group,
        })
    }

    /// The package with the files of another part of it, which another
    /// folder ships: a file both ship is written alike in both.
    fn add(&mut self, files: Files) -> Result<(), String> {
        let mut all = self.files.clone();
        for (name, (text, from)) in files {
            match all.get(&name) {
                Some((other, other_from)) if *other != text => {
                    return Err(format!(
                        "`{name}` of `{}` is written two ways, in `{}` and in `{}`: a file of a \
                         package is written alike wherever it is shipped",
                        self.id,
                        other_from.display(),
                        from.display()
                    ));
                }
                Some(_) => {}
                None => {
                    all.insert(name, (text, from));
                }
            }
        }
        *self = Self::parsed(all)?;
        Ok(())
    }

    /// Whether the package is the one `module` is an interface of, at its
    /// version, and declares that interface itself. One it only uses from
    /// another package is in WIT's parser's list too, with no name.
    fn declares(&self, module: &str) -> bool {
        let Ok(ImportModule::Interface { interface, .. }) = import_module(module) else {
            return false;
        };
        package_id(module).as_deref() == Some(self.id.as_str())
            && self
                .group
                .main
                .interfaces
                .iter()
                .any(|(_, own)| own.name.as_deref() == Some(interface))
    }
}

/// Why the build cannot type an import from `module`, an interface of a
/// package that no WIT folder ships as the module names it: the package is
/// shipped without that interface, at another version, or not at all. rk
/// writes no WIT for a package: its WIT is its own.
fn unshipped(module: &str, shipped: &[Shipped]) -> String {
    let id = package_id(module).unwrap_or_default();
    let name = id.split('@').next().unwrap_or_default();
    let interface = match import_module(module) {
        Ok(ImportModule::Interface { interface, .. }) => interface,
        _ => module,
    };
    if let Some(package) = shipped.iter().find(|package| package.id == id) {
        return format!(
            "the module imports `{module}`, and the WIT folders ship `{}` without the \
             interface `{interface}`",
            package.id
        );
    }
    if let Some(other) = shipped
        .iter()
        .find(|package| package.id.split('@').next() == Some(name))
    {
        return format!(
            "the module imports `{module}`, and a WIT folder ships the package as `{}`, at \
             another version",
            other.id
        );
    }
    format!(
        "the module imports `{module}`, which no `wit/` folder of the workspace or of a \
         library describes"
    )
}

/// The packages the WIT `folders` hold: each folder's own, from its files,
/// and one from each folder in its `deps/`, as WIT's tools lay them out.
/// The parts of one package that several folders ship are one package, of
/// all their files: a library ships the files of a package that declare
/// what it imports, as the standard library ships WASI's.
fn shipped(folders: &[std::path::PathBuf]) -> Result<Vec<Shipped>, String> {
    let mut found: Vec<Shipped> = Vec::new();
    for folder in folders.iter().filter(|folder| folder.is_dir()) {
        let deps = folder.join("deps");
        // A folder may ship the packages in its `deps/` alone.
        let own = std::fs::read_dir(folder)
            .map_err(|e| format!("reading `{}`: {e}", folder.display()))?
            .filter_map(|entry| Some(entry.ok()?.path()))
            .any(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "wit"));
        let mut packages: Vec<_> = own.then(|| folder.clone()).into_iter().collect();
        if deps.is_dir() {
            let mut inside: Vec<_> = std::fs::read_dir(&deps)
                .map_err(|e| format!("reading `{}`: {e}", deps.display()))?
                .filter_map(|entry| Some(entry.ok()?.path()))
                .filter(|path| path.is_dir())
                .collect();
            inside.sort();
            packages.extend(inside);
        }
        for package in packages {
            let read = read_package(&package)?;
            if wit::packages().any(|own| own.id == read.id) {
                return Err(format!(
                    "`{}` holds `{}`, which rk declares itself: a library's `wit/` holds \
                     the packages it calls",
                    package.display(),
                    read.id
                ));
            }
            match found.iter_mut().find(|other| other.id == read.id) {
                Some(other) => other.add(read.files)?,
                None => found.push(read),
            }
        }
    }
    Ok(found)
}

/// The package, or the part of one, written in the `.wit` files of the
/// folder `from`. A checkout's line endings do not count.
fn read_package(from: &std::path::Path) -> Result<Shipped, String> {
    let mut files = Files::new();
    for entry in
        std::fs::read_dir(from).map_err(|e| format!("reading `{}`: {e}", from.display()))?
    {
        let path = entry
            .map_err(|e| format!("reading `{}`: {e}", from.display()))?
            .path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.is_file() && name.ends_with(".wit") {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("reading `{}`: {e}", path.display()))?;
            files.insert(
                name.to_string(),
                (text.replace("\r\n", "\n"), from.to_path_buf()),
            );
        }
    }
    Shipped::parsed(files)
}

/// The name and version of the package `module` is an interface of:
/// `wasi:clocks/monotonic-clock@0.3.0` is in `wasi:clocks@0.3.0`.
fn package_id(module: &str) -> Option<String> {
    match import_module(module).ok()? {
        ImportModule::Interface {
            namespace,
            package,
            version,
            ..
        } => Some(match version {
            Some(version) => format!("{namespace}:{package}@{version}"),
            None => format!("{namespace}:{package}"),
        }),
        ImportModule::Plain(_) => None,
    }
}

/// How many bytes each import of `module` holds of its result
/// ([`Interfaces::held`]): the record of the fields its declaration reads
/// back, as rk lays them out. A sync call reads its outputs and its return
/// value; an async call its outputs; a future's read its value's fields.
fn held(module: &mir::MirModule) -> BTreeMap<(String, String), u32> {
    let record = |types: &mut dyn Iterator<Item = &MirType>| {
        let fields: Vec<_> = types
            .filter_map(wasm_codegen::canonical::canonical)
            .collect();
        wasm_codegen::canonical::record_layout(&fields).1
    };
    module
        .extern_functions
        .iter()
        .map(|ext| {
            let size = match module
                .async_calls
                .values()
                .find(|call| call.import == ext.name)
            {
                Some(call) => record(&mut call.results.iter().map(|(_, ty)| ty)),
                None => record(&mut ext.results()),
            };
            ((ext.module.to_string(), ext.import_name.to_string()), size)
        })
        .collect()
}

/// What each stream's read or write in `module` moves: a read, what its
/// buffer holds, a handle in each UDINT of an array and bytes otherwise; a
/// write, the bytes of its STRING.
fn moved(module: &mir::MirModule) -> BTreeMap<(String, String), Moved> {
    use hir::hir_ty::async_import::AsyncKind;
    module
        .async_calls
        .values()
        .filter_map(|call| {
            let moved = match (call.kind, call.results.first()) {
                (AsyncKind::StreamRead, Some((_, MirType::Array(array))))
                    if *array.element_type
                        == MirType::Elementary(mir::types::MirElementary::UDInt) =>
                {
                    Moved::Handles
                }
                (AsyncKind::StreamRead | AsyncKind::StreamWrite, _) => Moved::Bytes,
                _ => return None,
            };
            let ext = &module.extern_functions[mir::async_call::import_at(module, call)? as usize];
            Some(((ext.module.to_string(), ext.import_name.to_string()), moved))
        })
        .collect()
}

/// The functions `externs` import from plain modules, typed in WIT: what
/// HIR accepts on such an import has a WIT type and name.
fn host_functions(
    db: &dyn db::WorkspaceDataBase,
    externs: &[MirExternFunction],
) -> Result<Vec<HostFunction>, String> {
    let mut functions = Vec::new();
    // The component model's built-ins are no interface's.
    for ext in externs
        .iter()
        .filter(|ext| !names_a_package(&ext.module) && ext.module != mir::async_call::ROOT)
    {
        let declared_by = ext.name.text(db).to_string();
        let untyped = |what: &str| {
            format!("internal compiler error: {what} of `{declared_by}` has no WIT type")
        };
        let mut params = Vec::new();
        for param in &ext.params {
            let name = param.name.text(db);
            let wit_name = hir::hir_def::pous::pragma::wit_parameter_name(name)
                .ok_or_else(|| untyped(&format!("the name of `{name}`")))?;
            let ty = wit_type(&param.ty).ok_or_else(|| untyped(&format!("`{name}`")))?;
            params.push((wit_name, ty));
        }
        let mut outputs = Vec::new();
        for (name, ty) in &ext.out_results {
            let name = name.text(db);
            let wit_name = hir::hir_def::pous::pragma::wit_parameter_name(name)
                .ok_or_else(|| untyped(&format!("the name of `{name}`")))?;
            let ty = wit_type(ty).ok_or_else(|| untyped(&format!("`{name}`")))?;
            outputs.push((wit_name, ty));
        }
        let result = match &ext.return_type {
            Some(ty) => Some(wit_type(ty).ok_or_else(|| untyped("the return type"))?),
            None => None,
        };
        let name = ext.import_name.as_str();
        functions.push(HostFunction {
            declared_by,
            module: ext.module.to_string(),
            name: name
                .strip_prefix(mir::async_call::ASYNC_LOWER)
                .unwrap_or(name)
                .to_string(),
            params,
            outputs,
            result,
            asynchronous: ext.asynchronous,
        });
    }
    Ok(functions)
}

/// The WIT type a value of `ty` crosses an import as: an integer as the
/// integer of its width and sign, a duration or a date as the signed
/// integer it is held in, an enum or a subrange as what it is stored as.
/// The code that reads and writes it lays it out from the same table.
fn wit_type(ty: &MirType) -> Option<&'static str> {
    wasm_codegen::canonical::canonical(ty).map(|canonical| canonical.wit)
}

/// The record a function with VAR_OUTPUTs returns: its name, `-outputs`
/// after it.
pub fn record_name(function: &str) -> String {
    format!("{function}-outputs")
}

/// Refuses an interface one of whose functions returns a record named as
/// another of its functions is: a record and a function of one interface
/// are two names of it, which the component model compares without their
/// `-`.
fn records_named_apart(interface: &str, functions: &[(&str, bool)]) -> Result<(), String> {
    let names: BTreeMap<String, &str> = functions
        .iter()
        .map(|(name, _)| (wit_key(name), *name))
        .collect();
    for (name, _) in functions.iter().filter(|(_, outputs)| *outputs) {
        let record = record_name(name);
        if let Some(other) = names.get(&wit_key(&record)) {
            return Err(format!(
                "`{name}` returns its VAR_OUTPUTs as the record `{record}`, which cannot be \
                 told from the function `{other}` of `{interface}`"
            ));
        }
    }
    Ok(())
}

/// Wraps `core` into a component, its world in the package named after
/// `project`, the functions it imports from plain modules and those it
/// exports typed by `interfaces`, those it imports from a package's
/// interface by the WIT that `interfaces` names. `Err` says what the module
/// imports that rk cannot type.
pub fn wrap(core: &[u8], project: &str, interfaces: &Interfaces) -> Result<Component, String> {
    let hosts = &interfaces.imports;
    let imports = imported_functions(core)?;
    let shipped = shipped(&interfaces.wit)?;
    // A package rk declares, or one a WIT folder ships, types the module.
    let typed = |module: &str| {
        wit::package_of(module).is_some() || shipped.iter().any(|package| package.declares(module))
    };
    let mut carried = BTreeSet::new();
    let mut own: BTreeMap<&str, Vec<&HostFunction>> = BTreeMap::new();
    for (module, names) in &imports {
        // The component model's built-ins a block checks its call with.
        if module == mir::async_call::ROOT {
            continue;
        }
        if typed(module) {
            carried.insert(module.as_str());
            continue;
        }
        if names_a_package(module) {
            return Err(unshipped(module, &shipped));
        }
        for name in names {
            own.entry(module)
                .or_default()
                .push(host_function(hosts, module, name)?);
        }
    }

    for (module, functions) in &own {
        let named: Vec<(&str, bool)> = functions
            .iter()
            .map(|function| (function.name.as_str(), !function.outputs.is_empty()))
            .collect();
        records_named_apart(module, &named)?;
    }
    let exported: Vec<(&str, bool)> = interfaces
        .exports
        .iter()
        .map(|function| (function.name.as_str(), !function.outputs.is_empty()))
        .collect();
    records_named_apart(&interfaces.export_interface, &exported)?;

    if !interfaces.exports.is_empty() && own.contains_key(interfaces.export_interface.as_str()) {
        return Err(format!(
            "the module imports from `{0}`, and exports its FUNCTIONs in the interface `{0}`, \
             named after the project",
            interfaces.export_interface
        ));
    }
    let world = world_text(project, &carried, &own, interfaces);
    let mut resolve = wit_parser::Resolve {
        // WASI's time zone is `@unstable(feature = clocks-timezone)`.
        all_features: true,
        ..Default::default()
    };
    let mut deps = wit::packages()
        .map(|package| parse(package.dir, package.files))
        .collect::<Result<Vec<_>, _>>()?;
    deps.extend(shipped.into_iter().map(|package| package.group));
    let main = parse(".", &[("world.wit", world.as_str())])?;
    let package = resolve
        .push_groups(main, deps)
        .map_err(|e| format!("resolving the component's WIT: {e:#}"))?;
    let world_id = resolve
        .select_world(&[package], Some(WORLD))
        .map_err(|e| format!("selecting the component's world: {e:#}"))?;
    let mut sizes = wit_parser::SizeAlign::default();
    sizes
        .fill(&resolve)
        .map_err(|e| format!("laying out the component's WIT types: {e:#}"))?;
    for module in &carried {
        as_its_wit_declares(&resolve, &sizes, module, &imports[*module], interfaces)?;
    }

    let mut module = core.to_vec();
    wit_component::embed_component_metadata(
        &mut module,
        &resolve,
        world_id,
        wit_component::StringEncoding::UTF8,
        false,
    )
    .map_err(|e| format!("typing the module with its world: {e:#}"))?;
    let bytes = wit_component::ComponentEncoder::default()
        .validate(true)
        .module(&module)
        .and_then(|encoder| encoder.encode())
        .map_err(|e| format!("wrapping the module into a component: {e:#}"))?;

    Ok(Component { bytes, world })
}

/// What `core` imports, by module: each function's name.
fn imported_functions(core: &[u8]) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut imports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for payload in wasmparser::Parser::new(0).parse_all(core) {
        let payload = payload.map_err(|e| format!("reading the module: {e}"))?;
        if let wasmparser::Payload::ImportSection(reader) = payload {
            for import in reader.into_imports() {
                let import = import.map_err(|e| format!("reading the module's imports: {e}"))?;
                imports
                    .entry(import.module.to_string())
                    .or_default()
                    .insert(import.name.to_string());
            }
        }
    }
    Ok(imports)
}

/// The function `module` exports as `name`, as `hosts` types it. Two
/// FUNCTIONs may import one function, typed alike: the host has one.
fn host_function<'a>(
    hosts: &'a [HostFunction],
    module: &str,
    name: &str,
) -> Result<&'a HostFunction, String> {
    // A function a block imports is the same function, lowered async.
    let name = name
        .strip_prefix(mir::async_call::ASYNC_LOWER)
        .unwrap_or(name);
    let mut declared = hosts
        .iter()
        .filter(|host| host.module == module && host.name == name);
    let first = declared.next().ok_or_else(|| {
        format!("internal compiler error: no FUNCTION declares `{module}` `{name}`")
    })?;
    let types = |host: &HostFunction| {
        (
            host.params.iter().map(|(_, ty)| *ty).collect::<Vec<_>>(),
            host.outputs.iter().map(|(_, ty)| *ty).collect::<Vec<_>>(),
            host.result,
            host.asynchronous,
        )
    };
    match declared.find(|other| types(other) != types(first)) {
        Some(other) => Err(format!(
            "`{}` and `{}` import `{name}` from `{module}` with two signatures, and the \
             host has one function under that name",
            first.declared_by, other.declared_by
        )),
        None => Ok(first),
    }
}

/// Refuses an import from `module`, an interface of a package, that the
/// package's WIT does not declare as the declaration says: a function the interface does not have, a
/// resource's among them, under the name WIT gives it (`[method]R.f`); the
/// drop of a resource the interface does not have; a stream's
/// or a future's built-in whose number names none of the function's streams
/// and futures, as wit-component numbers them, or one of the other kind; a
/// stream's read or write that moves what the stream does not carry
/// ([`Interfaces::moved`]), whose host would write elements of another width
/// than the buffer's; a result or a future read's value, laid out as
/// `sizes` says, in more bytes than the declaration's outputs take
/// ([`Interfaces::held`]): the host writes one that is not passed as values
/// where the call points, past outputs that are too few;
/// and `{extern async}`, which a FUNCTION_BLOCK takes, on a function that is
/// not an `async func`, or a plain `{extern}` on one. A scan never waits: a
/// FUNCTION's call of an `async func` traps when the host does not answer
/// at once, and an async call of any other gains nothing. Each error names
/// the POU that declares the import ([`Interfaces::declared`]).
fn as_its_wit_declares(
    resolve: &wit_parser::Resolve,
    sizes: &wit_parser::SizeAlign,
    module: &str,
    names: &BTreeSet<String>,
    interfaces: &Interfaces,
) -> Result<(), String> {
    let declared = &interfaces.declared;
    let size = |ty: &wit_parser::Type| sizes.size(ty).size_wasm32() as u32;
    let Some(interface) = resolve
        .interfaces
        .iter()
        .find(|(id, _)| resolve.id_of(*id).as_deref() == Some(module))
        .map(|(_, interface)| interface)
    else {
        return Ok(());
    };
    for name in names {
        let by = declared
            .get(&(module.to_string(), name.clone()))
            .map_or("a POU", String::as_str);
        let (asynchronous, imported) = match name.strip_prefix(mir::async_call::ASYNC_LOWER) {
            Some(imported) => (true, imported),
            None => (false, name.as_str()),
        };
        // `[resource-drop]R`: a resource the interface declares.
        if let Some(resource) = hir::hir_def::pous::pragma::resource_drop_name(imported) {
            let declared = interface.types.get(resource).is_some_and(|id| {
                matches!(resolve.types[*id].kind, wit_parser::TypeDefKind::Resource)
            });
            if !declared {
                return Err(format!(
                    "`{by}` imports `{imported}` of `{module}`, and its package's WIT declares \
                     no resource `{resource}` there"
                ));
            }
            continue;
        }
        let builtin = hir::hir_def::pous::pragma::builtin_name(imported);
        let function = builtin.map_or(imported, |(_, function)| function);
        let Some(wit) = interface.functions.get(function) else {
            return Err(format!(
                "`{by}` imports `{imported}` of `{module}`, and its package's WIT declares \
                 no function `{function}` there"
            ));
        };
        if let Some((builtin, _)) = builtin {
            // `[stream-new-0]f`: the number after the built-in's name.
            let number = imported[1..]
                .split_once(']')
                .and_then(|(inside, _)| inside.rsplit_once('-'))
                .and_then(|(_, number)| number.parse::<usize>().ok());
            let types = wit.find_futures_and_streams(resolve);
            let Some(&id) = number.and_then(|number| types.get(number)) else {
                let numbered = match types.len() {
                    0 => "has no stream or future".to_string(),
                    count => format!("numbers its streams and futures from 0 to {}", count - 1),
                };
                return Err(format!(
                    "`{by}` imports `{imported}` of `{module}`, and the type of `{function}` \
                     {numbered}"
                ));
            };
            let number = number.unwrap_or_default();
            let found = match (builtin.starts_with("stream"), &resolve.types[id].kind) {
                (true, wit_parser::TypeDefKind::Stream(payload)) => {
                    let Some(&moved) = interfaces.moved.get(&(module.to_string(), name.clone()))
                    else {
                        continue;
                    };
                    let carried = carried(resolve, *payload);
                    if carried == Some(moved) {
                        continue;
                    }
                    let carries = match (carried, builtin) {
                        (Some(Moved::Bytes), _) => {
                            "bytes, which a STRING or an ARRAY OF BYTE holds"
                        }
                        (Some(Moved::Handles), "stream-read") => {
                            "a resource's handles, which an ARRAY OF UDINT holds"
                        }
                        (Some(Moved::Handles), _) => {
                            "a resource's handles, which no block writes yet"
                        }
                        (None, _) => "neither bytes nor handles, which no block moves yet",
                    };
                    return Err(format!(
                        "`{by}` imports `{imported}` of `{module}`, and number {number} of the \
                         streams and futures of `{function}`'s type carries {carries}"
                    ));
                }
                (false, wit_parser::TypeDefKind::Future(payload)) => {
                    // A read's value lands where its block points, over
                    // what its outputs take.
                    if builtin == "future-read"
                        && let Some(&held) =
                            interfaces.held.get(&(module.to_string(), name.clone()))
                    {
                        let written = payload.as_ref().map_or(0, size);
                        if written > held {
                            return Err(format!(
                                "`{by}` imports `{imported}` of `{module}`, whose value takes \
                                 {written} bytes, where its outputs take {held}: declare \
                                 outputs for the whole value"
                            ));
                        }
                    }
                    continue;
                }
                (true, _) => "a future",
                (false, _) => "a stream",
            };
            return Err(format!(
                "`{by}` imports `{imported}` of `{module}`, and number {number} of the streams \
                 and futures of `{function}`'s type is {found}"
            ));
        }
        match (asynchronous, wit.kind.is_async()) {
            (true, false) => {
                return Err(format!(
                    "`{by}` imports `{function}` of `{module}` with {{extern async}}, and \
                     its package's WIT declares it a `func`: import it with {{extern}} on a \
                     FUNCTION"
                ));
            }
            (false, true) => {
                return Err(format!(
                    "`{by}` imports `{function}` of `{module}` with {{extern}}, and its \
                     package's WIT declares it an `async func`: import it with {{extern async}} on a \
                     FUNCTION_BLOCK"
                ));
            }
            _ => {}
        }
        // The outputs hold the whole result: one past a call's values, and
        // an async call's, the host writes where the call points, over what
        // they take.
        if let (Some(result), Some(&held)) = (
            wit.result.as_ref(),
            interfaces.held.get(&(module.to_string(), name.clone())),
        ) {
            let written = size(result);
            if written > held {
                return Err(format!(
                    "`{by}` imports `{imported}` of `{module}`, whose result takes {written} \
                     bytes, where its outputs take {held}: declare outputs for the whole result"
                ));
            }
        }
    }
    Ok(())
}

/// What a stream of `payload` carries that a block moves: bytes, or a
/// resource's handles.
fn carried(resolve: &wit_parser::Resolve, payload: Option<wit_parser::Type>) -> Option<Moved> {
    match payload? {
        wit_parser::Type::U8 => Some(Moved::Bytes),
        wit_parser::Type::Id(id) => {
            matches!(resolve.types[id].kind, wit_parser::TypeDefKind::Handle(_))
                .then_some(Moved::Handles)
        }
        _ => None,
    }
}

/// One level of the world's layout: two spaces, as WASI's own files and
/// wasm-tools' printers indent WIT.
const INDENT: &str = "  ";

/// The world: what the component imports, and the control interface. An
/// interface named by a plain name is declared in the world itself.
fn world_text(
    project: &str,
    carried: &BTreeSet<&str>,
    own: &BTreeMap<&str, Vec<&HostFunction>>,
    interfaces: &Interfaces,
) -> String {
    let (one, two) = (INDENT, INDENT.repeat(2));
    let mut text = format!("package local:{};\n\nworld {WORLD} {{\n", wit_name(project));
    // A package's name may be spelled like a keyword too.
    for interface in carried {
        text.push_str(&format!("{one}import {};\n", interface_path(interface)));
    }
    // Every other import is a plain module's, its interface written from
    // the FUNCTIONs that import it.
    for (module, functions) in own {
        text.push_str(&format!(
            "{one}import {}: interface {{\n{}{one}}}\n",
            wit_id(module),
            functions_text(functions, &two)
        ));
    }
    text.push_str(&format!("{one}export {};\n", wit::CONTROL_INTERFACE));
    if !interfaces.exports.is_empty() {
        text.push_str(&format!(
            "{one}export {}: interface {{\n",
            wit_id(&interfaces.export_interface)
        ));
        for function in &interfaces.exports {
            let result = match function.outputs.is_empty() {
                true => function.result.unwrap_or("_").to_string(),
                false => {
                    text.push_str(&record_text(
                        &function.name,
                        &function.outputs,
                        function.result,
                        &two,
                    ));
                    record_name(&function.name)
                }
            };
            text.push_str(&format!(
                "{two}{}: func({}) -> result<{result}, string>;\n",
                wit_id(&function.name),
                params_text(&function.params),
            ));
        }
        text.push_str(&format!("{one}}}\n"));
    }
    text.push_str("}\n");
    text
}

/// Parameters as WIT declares them.
fn params_text(params: &[(String, &'static str)]) -> String {
    let params: Vec<String> = params
        .iter()
        .map(|(name, ty)| format!("{}: {ty}", wit_id(name)))
        .collect();
    params.join(", ")
}

/// The record a function `name` with `outputs` returns, as WIT declares
/// it: each output, and the return value last under the function's name.
fn record_text(
    name: &str,
    outputs: &[(String, &'static str)],
    result: Option<&'static str>,
    indent: &str,
) -> String {
    let mut text = format!("{indent}record {} {{\n", record_name(name));
    let fields = outputs
        .iter()
        .map(|(field, ty)| (field.as_str(), *ty))
        .chain(result.map(|ty| (name, ty)));
    for (field, ty) in fields {
        text.push_str(&format!("{indent}{INDENT}{}: {ty},\n", wit_id(field)));
    }
    text.push_str(&format!("{indent}}}\n"));
    text
}

/// `functions` as WIT declares them, one a line, each after the record it
/// returns when it has VAR_OUTPUTs.
fn functions_text(functions: &[&HostFunction], indent: &str) -> String {
    let mut text = String::new();
    for function in functions {
        let result = match (function.outputs.is_empty(), function.result) {
            (true, None) => String::new(),
            (true, Some(ty)) => format!(" -> {ty}"),
            (false, _) => {
                text.push_str(&record_text(
                    &function.name,
                    &function.outputs,
                    function.result,
                    indent,
                ));
                format!(" -> {}", record_name(&function.name))
            }
        };
        let func = match function.asynchronous {
            true => "async func",
            false => "func",
        };
        text.push_str(&format!(
            "{indent}{}: {func}({}){result};\n",
            wit_id(&function.name),
            params_text(&function.params)
        ));
    }
    text
}

/// An interface name as a world imports it, each name in it written as
/// WIT reads it.
fn interface_path(module: &str) -> String {
    match import_module(module) {
        Ok(ImportModule::Interface {
            namespace,
            package,
            interface,
            version,
        }) => format!(
            "{}:{}/{}{}",
            wit_id(namespace),
            wit_id(package),
            wit_id(interface),
            version.map_or(String::new(), |version| format!("@{version}"))
        ),
        _ => module.to_string(),
    }
}

/// WIT's keywords, as the wit-parser rk builds with lexes them: a name
/// spelled like one is written after a `%`. A test reads that parser's
/// keyword table, so a version that adds one fails until it is here.
const KEYWORDS: &[&str] = &[
    "as",
    "async",
    "bool",
    "borrow",
    "char",
    "constructor",
    "enum",
    "error-context",
    "export",
    "f32",
    "f64",
    "flags",
    "from",
    "func",
    "future",
    "import",
    "include",
    "interface",
    "list",
    "map",
    "option",
    "own",
    "package",
    "record",
    "resource",
    "result",
    "s16",
    "s32",
    "s64",
    "s8",
    "static",
    "stream",
    "string",
    "tuple",
    "type",
    "u16",
    "u32",
    "u64",
    "u8",
    "use",
    "variant",
    "with",
    "world",
];

/// `name` as WIT text writes it: after a `%` when it is a keyword.
fn wit_id(name: &str) -> Cow<'_, str> {
    match KEYWORDS.contains(&name) {
        true => Cow::Owned(format!("%{name}")),
        false => Cow::Borrowed(name),
    }
}

/// A package written in `files`, parsed.
fn parse(dir: &str, files: &[(&str, &str)]) -> Result<wit_parser::UnresolvedPackageGroup, String> {
    let mut map = wit_parser::SourceMap::new();
    for (file, text) in files {
        map.push_str(&format!("{dir}/{file}"), text.to_string());
    }
    map.parse()
        .map_err(|(_, e)| format!("parsing the WIT in `{dir}`: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A project's name becomes the world's package name, in the words WIT
    /// spells, or `plc` when it spells none.
    #[test]
    fn a_project_name_becomes_a_wit_name() {
        assert_eq!(wit_name("plant"), "plant");
        assert_eq!(wit_name("My_Plant"), "my-plant");
        assert_eq!(wit_name("line 2"), "line2");
        assert_eq!(wit_name("2nd"), "plc");
        assert_eq!(wit_name("_"), "plc");
    }

    /// The keywords are wit-parser's: each one, as a function's name, only
    /// parses after a `%`, and any other name parses bare.
    #[test]
    fn a_name_spelled_like_a_keyword_is_escaped() {
        for keyword in KEYWORDS {
            let bare = format!("package a:b;\ninterface i {{\n    {keyword}: func();\n}}\n");
            assert!(
                parse("k", &[("k.wit", &bare)]).is_err(),
                "`{keyword}` is a keyword"
            );
            let escaped = format!(
                "package a:b;\ninterface i {{\n    {}: func();\n}}\n",
                wit_id(keyword)
            );
            parse("k", &[("k.wit", &escaped)]).unwrap_or_else(|e| panic!("{keyword}: {e}"));
        }
        assert_eq!(wit_id("read-coils"), "read-coils");
    }

    /// The source of the wit-parser this crate builds with, as cargo
    /// resolved it: its lexer. Cargo is asked for the packages of its own
    /// platform only, whose sources the build has already, so it needs no
    /// network.
    fn wit_parser_lexer() -> String {
        let cargo = |args: &[&str]| {
            let out = std::process::Command::new(env!("CARGO"))
                .args(args)
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .output()
                .expect("cargo runs");
            assert!(
                out.status.success(),
                "cargo {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).expect("cargo writes UTF-8")
        };
        let version = cargo(&["-vV"]);
        let host = version
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .expect("cargo's host");
        let metadata: serde_json::Value = serde_json::from_str(&cargo(&[
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--filter-platform",
            host,
        ]))
        .expect("cargo's metadata");
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let packages = metadata["packages"].as_array().expect("the packages");
        let this = packages
            .iter()
            .find(|package| {
                package["manifest_path"]
                    .as_str()
                    .is_some_and(|path| std::path::Path::new(path) == manifest)
            })
            .expect("this crate");
        let node = metadata["resolve"]["nodes"]
            .as_array()
            .expect("the resolved graph")
            .iter()
            .find(|node| node["id"] == this["id"])
            .expect("this crate's node");
        let parser = node["deps"]
            .as_array()
            .expect("its dependencies")
            .iter()
            .find(|dep| dep["name"] == "wit_parser")
            .expect("wit-parser among them");
        let package = packages
            .iter()
            .find(|package| package["id"] == parser["pkg"])
            .expect("wit-parser's package");
        let root = std::path::Path::new(package["manifest_path"].as_str().expect("a path"))
            .parent()
            .expect("its folder")
            .to_path_buf();
        std::fs::read_to_string(root.join("src/ast/lex.rs"))
            .unwrap_or_else(|e| panic!("wit-parser's lexer in {}: {e}", root.display()))
    }

    /// The keywords are all of wit-parser's: those of the `match` its lexer
    /// tells a keyword from a name with, the arms before `_ => Id`, that a
    /// name can be spelled as. A keyword missing here would be written bare
    /// and refused; `_`, which stands for no type, is no name.
    #[test]
    fn the_keywords_are_all_of_wit_parser_s() {
        let lexer = wit_parser_lexer();
        let end = lexer
            .find("_ => Id,")
            .expect("the lexer's keyword table, which ends with `_ => Id,`");
        let mut table: Vec<&str> = lexer[..end]
            .lines()
            .rev()
            .map(str::trim)
            .skip_while(|line| line.is_empty())
            .map_while(|line| {
                let word = line.strip_prefix('"')?;
                let (word, rest) = word.split_once('"')?;
                rest.trim_start().starts_with("=>").then_some(word)
            })
            .filter(|word| word.starts_with(|c: char| c.is_ascii_lowercase()))
            .collect();
        table.sort_unstable();
        let mut ours = KEYWORDS.to_vec();
        ours.sort_unstable();
        assert!(!table.is_empty(), "the lexer's keyword table is empty");
        assert_eq!(
            ours, table,
            "KEYWORDS must be the keywords of wit-parser's lexer, src/ast/lex.rs"
        );
    }
}
