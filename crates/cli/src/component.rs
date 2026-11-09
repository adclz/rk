// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A built module wrapped into a component, and the WIT that types it.
//!
//! The world is generated from the module at each build: it imports the
//! WASI interfaces the module calls, as WASI declares them, and exports the
//! control interface every module has (`rk:plc/control`). Binaryen reads
//! core modules only, so the wrapping is the build's last step, after
//! `wasm-opt`.

use std::collections::BTreeSet;

use debug_format::wit;

/// The world every component exports its control interface from.
pub const WORLD: &str = "plc";

/// A component, and the WIT folder that types it: `world.wit`, and under
/// `deps/` each package the world names, by path in the folder.
pub struct Component {
    pub bytes: Vec<u8>,
    pub wit: Vec<(String, &'static str)>,
    pub world: String,
}

/// Wraps `core` into a component, its world in the package named after
/// `project`. `Err` says what the module imports that rk cannot type.
pub fn wrap(core: &[u8], project: &str) -> Result<Component, String> {
    let imports = imported_interfaces(core)?;
    if let Some(interface) = imports
        .iter()
        .find(|interface| wit::package_of(interface).is_none())
    {
        return Err(format!(
            "the module imports `{interface}`, which no WIT package rk carries describes"
        ));
    }

    let world = world_text(project, &imports);
    let mut resolve = wit_parser::Resolve {
        // WASI's time zone is `@unstable(feature = clocks-timezone)`.
        all_features: true,
        ..Default::default()
    };
    let deps = wit::packages()
        .map(|package| parse(package.dir, package.files))
        .collect::<Result<Vec<_>, _>>()?;
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
            files.push((format!("deps/{}/{file}", package.dir), *text));
        }
        if let Some(license) = package.license {
            files.push((format!("deps/{}/LICENSE.md", package.dir), license));
        }
    }
    Ok(Component {
        bytes,
        wit: files,
        world,
    })
}

/// The interfaces `core` imports functions from, by name: each module name
/// of its imports.
fn imported_interfaces(core: &[u8]) -> Result<BTreeSet<String>, String> {
    let mut interfaces = BTreeSet::new();
    for payload in wasmparser::Parser::new(0).parse_all(core) {
        let payload = payload.map_err(|e| format!("reading the module: {e}"))?;
        if let wasmparser::Payload::ImportSection(reader) = payload {
            for import in reader.into_imports() {
                let import = import.map_err(|e| format!("reading the module's imports: {e}"))?;
                interfaces.insert(import.module.to_string());
            }
        }
    }
    Ok(interfaces)
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

/// The world: what the component imports, and the control interface.
fn world_text(project: &str, imports: &BTreeSet<String>) -> String {
    let mut text = format!(
        "// The world of `{project}`, written by rk at each build.\npackage local:{};\n\nworld {WORLD} {{\n",
        wit_name(project)
    );
    for interface in imports {
        text.push_str(&format!("    import {interface};\n"));
    }
    text.push_str(&format!("    export {};\n}}\n", wit::CONTROL_INTERFACE));
    text
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
}
