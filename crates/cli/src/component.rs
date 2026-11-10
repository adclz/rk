// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A built module wrapped into a component, and the WIT that types it.
//!
//! The world is generated from the module at each build: it imports the
//! WASI interfaces the module calls, as WASI declares them, the host's own
//! interfaces, typed from the `{extern}` FUNCTIONs that import them, and
//! exports the control interface every module has (`rk:plc/control`).
//! Binaryen reads core modules only, so the wrapping is the build's last
//! step, after `wasm-opt`.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use debug_format::wit;
use hir::hir_def::pous::pragma::{ImportModule, import_module, is_wasi_module};
use mir::function::MirExternFunction;
use mir::types::{MirElementary, MirType};

/// The world every component exports its control interface from.
pub const WORLD: &str = "plc";

/// A component, and the WIT folder that types it: `world.wit`, and under
/// `deps/` each package the world names, by path in the folder.
pub struct Component {
    pub bytes: Vec<u8>,
    pub wit: Vec<(String, Cow<'static, str>)>,
    pub world: String,
}

/// A function the module imports from a module that is not WASI's, typed
/// in WIT from the `{extern}` FUNCTION that declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFunction {
    /// The FUNCTION, as the module names it.
    pub declared_by: String,
    /// `modbus`, or `acme:modbus/rtu@1.0.0`.
    pub module: String,
    /// `read-coils`.
    pub name: String,
    /// Each parameter's WIT name and type.
    pub params: Vec<(String, &'static str)>,
    pub result: Option<&'static str>,
}

/// The functions `externs` import from modules other than WASI's, typed
/// in WIT: what HIR accepts on such an import has a WIT type and name.
pub fn host_functions(
    db: &dyn db::WorkspaceDataBase,
    externs: &[MirExternFunction],
) -> Result<Vec<HostFunction>, String> {
    let mut functions = Vec::new();
    for ext in externs.iter().filter(|ext| !is_wasi_module(&ext.module)) {
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
        if !ext.out_results.is_empty() {
            return Err(untyped("a VAR_OUTPUT"));
        }
        let result = match &ext.return_type {
            Some(ty) => Some(wit_type(ty).ok_or_else(|| untyped("the return type"))?),
            None => None,
        };
        functions.push(HostFunction {
            declared_by,
            module: ext.module.to_string(),
            name: ext.import_name.to_string(),
            params,
            result,
        });
    }
    Ok(functions)
}

/// The WIT type a value of `ty` crosses an import as: an integer as the
/// integer of its width and sign, a duration or a date as the signed
/// integer it is held in, an enum or a subrange as what it is stored as.
fn wit_type(ty: &MirType) -> Option<&'static str> {
    let elementary = match ty {
        MirType::String { .. } => return Some("string"),
        MirType::Elementary(elementary) => *elementary,
        MirType::Enum(enumeration) => enumeration.storage,
        MirType::Subrange(subrange) => subrange.base,
        _ => return None,
    };
    Some(match elementary {
        MirElementary::Bool => "bool",
        MirElementary::SInt => "s8",
        MirElementary::Int => "s16",
        MirElementary::DInt | MirElementary::Time | MirElementary::Date | MirElementary::Tod => {
            "s32"
        }
        MirElementary::LInt
        | MirElementary::LTime
        | MirElementary::LDate
        | MirElementary::LTod
        | MirElementary::DateAndTime
        | MirElementary::LDateTime => "s64",
        MirElementary::USInt | MirElementary::Byte => "u8",
        MirElementary::UInt | MirElementary::Word => "u16",
        MirElementary::UDInt | MirElementary::DWord => "u32",
        MirElementary::ULInt | MirElementary::LWord => "u64",
        MirElementary::Real => "f32",
        MirElementary::LReal => "f64",
        MirElementary::Char => "char",
    })
}

/// Wraps `core` into a component, its world in the package named after
/// `project`, the functions it imports from modules other than WASI's typed
/// by `hosts`. `Err` says what the module imports that rk cannot type.
pub fn wrap(core: &[u8], project: &str, hosts: &[HostFunction]) -> Result<Component, String> {
    let imports = imported_functions(core)?;
    let mut carried = BTreeSet::new();
    let mut own: BTreeMap<&str, Vec<&HostFunction>> = BTreeMap::new();
    for (module, names) in &imports {
        if wit::package_of(module).is_some() {
            carried.insert(module.as_str());
            continue;
        }
        if is_wasi_module(module) {
            return Err(format!(
                "the module imports `{module}`, which no WIT package rk carries describes"
            ));
        }
        for name in names {
            own.entry(module)
                .or_default()
                .push(host_function(hosts, module, name)?);
        }
    }

    let packages = own_packages(&own)?;
    let world = world_text(project, &carried, &own);
    let mut resolve = wit_parser::Resolve {
        // WASI's time zone is `@unstable(feature = clocks-timezone)`.
        all_features: true,
        ..Default::default()
    };
    let mut deps = wit::packages()
        .map(|package| parse(package.dir, package.files))
        .collect::<Result<Vec<_>, _>>()?;
    for package in &packages {
        let files: Vec<(&str, &str)> = package
            .files
            .iter()
            .map(|(file, text)| (file.as_str(), text.as_str()))
            .collect();
        deps.push(parse(&package.dir, &files)?);
    }
    let main = parse(".", &[("world.wit", world.as_str())])?;
    let package = resolve
        .push_groups(main, deps)
        .map_err(|e| format!("resolving the component's WIT: {e:#}"))?;
    let world_id = resolve
        .select_world(&[package], Some(WORLD))
        .map_err(|e| format!("selecting the component's world: {e:#}"))?;

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

    let named = named_packages(&resolve, package);
    let mut files = Vec::new();
    for package in wit::packages().filter(|package| named.contains(package.id)) {
        for (file, text) in package.files {
            files.push((format!("deps/{}/{file}", package.dir), Cow::Borrowed(*text)));
        }
        if let Some(license) = package.license {
            files.push((
                format!("deps/{}/LICENSE.md", package.dir),
                Cow::Borrowed(license),
            ));
        }
    }
    for package in packages {
        for (file, text) in package.files {
            files.push((format!("deps/{}/{file}", package.dir), Cow::Owned(text)));
        }
    }
    Ok(Component {
        bytes,
        wit: files,
        world,
    })
}

/// The component's WIT folder, `wit/` beside `output`: `world.wit`, and
/// each package it names under `deps/`. A package's folder is written
/// whole, so a file an earlier build left in it goes; nothing else in the
/// folder is touched.
pub fn write_wit(output: &std::path::Path, component: &Component) -> Result<(), String> {
    let dir = output
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("wit");
    let packages: BTreeSet<_> = component
        .wit
        .iter()
        .filter_map(|(path, _)| std::path::Path::new(path).parent())
        .collect();
    for package in packages {
        let package = dir.join(package);
        match std::fs::remove_dir_all(&package) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(format!("removing {}: {e}", package.display()));
            }
            _ => {}
        }
    }
    let files = std::iter::once(("world.wit".to_string(), component.world.as_str())).chain(
        component
            .wit
            .iter()
            .map(|(path, text)| (path.clone(), text.as_ref())),
    );
    for (path, text) in files {
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating directory {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    Ok(())
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
    let mut declared = hosts
        .iter()
        .filter(|host| host.module == module && host.name == name);
    let first = declared.next().ok_or_else(|| {
        format!("internal compiler error: no FUNCTION declares `{module}` `{name}`")
    })?;
    let types = |host: &HostFunction| {
        (
            host.params.iter().map(|(_, ty)| *ty).collect::<Vec<_>>(),
            host.result,
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

/// A package rk writes for the interfaces the module imports from it.
struct OwnPackage {
    /// Its folder under `deps/`.
    dir: String,
    /// One file per interface.
    files: Vec<(String, String)>,
}

/// A package's interfaces the module imports, by version: each one's name,
/// and the module that names it.
type Versions<'a> = BTreeMap<Option<&'a str>, Vec<(&'a str, &'a str)>>;

/// The packages of the interfaces in `own` named by an interface name,
/// `acme:modbus/rtu@1.0.0`, each in its folder under `deps/`: the package
/// folder, `acme-modbus`, with `@1.0.0` after it when the module imports
/// from the package at two versions.
fn own_packages(own: &BTreeMap<&str, Vec<&HostFunction>>) -> Result<Vec<OwnPackage>, String> {
    let mut by_package: BTreeMap<(&str, &str), Versions> = BTreeMap::new();
    for module in own.keys() {
        if let Ok(ImportModule::Interface {
            namespace,
            package,
            interface,
            version,
        }) = import_module(module)
        {
            if let Some(carried) = wit::packages().find(|carried| {
                carried.id.split('@').next() == Some(&format!("{namespace}:{package}"))
            }) {
                return Err(format!(
                    "the module imports `{module}`, from the package `{}` rk declares itself",
                    carried.id
                ));
            }
            by_package
                .entry((namespace, package))
                .or_default()
                .entry(version)
                .or_default()
                .push((interface, module));
        }
    }
    let mut packages = Vec::new();
    for ((namespace, package), versions) in by_package {
        let several = versions.len() > 1;
        for (version, interfaces) in versions {
            let mut dir = format!("{namespace}-{package}");
            let mut name = format!("{}:{}", wit_id(namespace), wit_id(package));
            if let Some(version) = version {
                name.push_str(&format!("@{version}"));
                if several {
                    dir.push_str(&format!("@{version}"));
                }
            }
            let files = interfaces
                .into_iter()
                .map(|(interface, module)| {
                    let text = format!(
                        "// What the module imports from `{module}`, written by rk at each build.\n\
                         package {name};\n\ninterface {} {{\n{}}}\n",
                        wit_id(interface),
                        functions_text(&own[module], "    ")
                    );
                    (format!("{interface}.wit"), text)
                })
                .collect();
            packages.push(OwnPackage { dir, files });
        }
    }
    Ok(packages)
}

/// The packages `package` names, and those they name in turn, by name and
/// version (`wasi:clocks@0.3.0`): what a tool reading the world needs.
fn named_packages(
    resolve: &wit_parser::Resolve,
    package: wit_parser::PackageId,
) -> BTreeSet<String> {
    let mut named = BTreeSet::new();
    let mut next = vec![package];
    while let Some(id) = next.pop() {
        for dep in resolve.package_direct_deps(id) {
            if named.insert(resolve.packages[dep].name.to_string()) {
                next.push(dep);
            }
        }
    }
    named
}

/// The world: what the component imports, and the control interface. An
/// interface named by a plain name is declared in the world itself.
fn world_text(
    project: &str,
    carried: &BTreeSet<&str>,
    own: &BTreeMap<&str, Vec<&HostFunction>>,
) -> String {
    let mut text = format!(
        "// The world of `{project}`, written by rk at each build.\npackage local:{};\n\nworld {WORLD} {{\n",
        wit_name(project)
    );
    for interface in carried {
        text.push_str(&format!("    import {interface};\n"));
    }
    for (module, functions) in own {
        match import_module(module) {
            Ok(ImportModule::Plain(name)) => text.push_str(&format!(
                "    import {}: interface {{\n{}    }}\n",
                wit_id(name),
                functions_text(functions, "        ")
            )),
            _ => text.push_str(&format!("    import {};\n", interface_path(module))),
        }
    }
    text.push_str(&format!("    export {};\n}}\n", wit::CONTROL_INTERFACE));
    text
}

/// `functions` as WIT declares them, one a line.
fn functions_text(functions: &[&HostFunction], indent: &str) -> String {
    let mut text = String::new();
    for function in functions {
        let params: Vec<String> = function
            .params
            .iter()
            .map(|(name, ty)| format!("{}: {ty}", wit_id(name)))
            .collect();
        let result = function
            .result
            .map_or(String::new(), |ty| format!(" -> {ty}"));
        text.push_str(&format!(
            "{indent}{}: func({}){result};\n",
            wit_id(&function.name),
            params.join(", ")
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

/// WIT's keywords: a name spelled like one is written after a `%`.
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

/// `name` as a WIT name: its words lowercase, joined by `-`, a word that
/// starts with a digit kept on the one before (WIT starts each word with a
/// letter). A name that starts with a digit, or has no word, gives `plc`.
pub fn wit_name(name: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    for word in name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
    {
        let word = word.to_ascii_lowercase();
        match words.last_mut() {
            Some(last) if word.starts_with(|c: char| c.is_ascii_digit()) => last.push_str(&word),
            _ => words.push(word),
        }
    }
    match words.first() {
        Some(first) if first.starts_with(|c: char| c.is_ascii_lowercase()) => words.join("-"),
        _ => WORLD.to_string(),
    }
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
}
