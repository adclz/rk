// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! ST declarations for the WIT interfaces a world imports, as wit-bindgen
//! writes Rust for them: a FUNCTION for each function, an `{extern async}`
//! FUNCTION_BLOCK for each async one, a typed enum for each enum, for the
//! cases of each variant and for each flags type, which starts with no
//! flag set, a STRUCT for each record, each tuple WIT names and each
//! variant whose cases hold values ([`types`]), a TYPE for each alias of a
//! scalar or of a STRUCT, the drop of each resource, and the built-ins of
//! each stream's and future's type the interface takes or gives, once per
//! type, under the name of its first function whose type has it.
//!
//! A value of a STRUCT is taken as one input and given back whole: as the
//! FUNCTION's own value, or as an output. Its import is apart from it,
//! `<NAME>_RAW`. A record of scalars comes back in the import's outputs,
//! which rk lays out as the canonical ABI lays out the record, and
//! `<NAME>` puts each in its member; one passed in goes field by field. A
//! result that holds a variant, a record of records, or a value and an
//! error, comes back in words, as the canonical ABI lays it out, and
//! `<NAME>` reads each value out of them ([`lift`]). A parameter that
//! holds one is passed the same way: `<NAME>` packs its values into the
//! words `<NAME>_RAW` takes ([`lower`]).
//!
//! Each interface is a namespace inside its package's, inside the one its
//! package's namespace names: `wasi:clocks/monotonic-clock` is
//! `Wasi.Clocks.MonotonicClock`, and `acme:drive/axis` is
//! `Acme.Drive.Axis`. That root is INTERNAL, so that only the library that
//! holds the code reaches it, unless the declarations are written for
//! every program ([`Reach`]). Every declaration is in the namespace of the
//! interface it is imported from, the built-ins too, named after their
//! type and what carries it: `Wasi.Cli.Stdout.BYTES_STREAM_NEW`. A function
//! or a type ST cannot express yet is left out, and the file says why at
//! its end.
//!
//! ST compares names without case, and two WIT names may be written alike.
//! The names WIT gives come first: of two that ST takes for one, the later
//! function is left out, and a name the generator makes, a drop, a `_RAW`
//! import or a built-in, takes a `_` after it. So does a function named as
//! a type, since a program names the type, and a variable that would be
//! the name of its FUNCTION's value, or one its generated code uses
//! ([`names::Scope`]). Two members of a STRUCT that ST takes for one leave
//! its type out, and the functions that take or give it. Two interfaces
//! that would be one namespace, or one file, stop the generator.

pub mod lift;
pub mod lower;
pub mod names;
pub mod shapes;
pub mod types;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};

use wit_parser::abi::AbiVariant;
use wit_parser::{
    Docs, Function, FunctionKind, Int, InterfaceId, Resolve, SizeAlign, Type, TypeDefKind,
    TypeOwner, WorldId, WorldItem,
};

use names::Scope;
use shapes::{BYTES_AT_ONCE, HANDLES_AT_ONCE, STRING_LIMIT, Shape, Var};

/// The canonical ABI passes a function's parameters as values up to this
/// many, and in memory past it, which an import declared in ST does not.
const MAX_FLAT_PARAMS: usize = 16;

/// An async call takes its parameters as values up to this many, and as
/// the address of a tuple of them past it, which rk lays out field by field.
pub(crate) const MAX_FLAT_ASYNC_PARAMS: usize = 4;

/// A file of generated code.
#[derive(Debug)]
pub struct File {
    /// Its name, its package's then its interface's:
    /// `wasi-clocks-monotonic-clock.st`.
    pub name: String,
    pub text: String,
    /// What was left out of it.
    pub skipped: Vec<Skipped>,
}

/// A function or a type left out, and why.
#[derive(Debug, Clone)]
pub struct Skipped {
    /// What was left out, as the comment names it: a WIT name, or the ST
    /// names a built-in would have.
    pub item: String,
    pub reason: String,
}

/// How every file rk-bindgen writes starts: the comment that says what it
/// is generated from ([`header`]).
const GENERATED: &str = "/* Generated from the WIT of ";

/// Whether `text` is a file rk-bindgen wrote: it starts as every one does.
pub fn is_generated(text: &str) -> bool {
    text.starts_with(GENERATED)
}

/// The files [`write`] removed from a folder: the `.st` files rk-bindgen
/// wrote there before that it no longer generates, by name.
pub type Removed = Vec<String>;

/// Writes `files` into `folder`, making it when it is missing, and removes
/// the `.st` files it holds that rk-bindgen wrote before and no longer
/// generates. A file it did not write, one that does not start as
/// [`is_generated`] says, is left as it is, whatever its name: the folder
/// may be a library's own.
pub fn write(folder: &Path, files: &[File]) -> Result<Removed, String> {
    std::fs::create_dir_all(folder).map_err(|e| format!("making `{}`: {e}", folder.display()))?;
    let entries =
        std::fs::read_dir(folder).map_err(|e| format!("reading `{}`: {e}", folder.display()))?;
    let mut removed = Removed::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let stale = path.extension().is_some_and(|extension| extension == "st")
            && !files.iter().any(|file| file.name == name)
            && std::fs::read_to_string(&path).is_ok_and(|text| is_generated(&text));
        if stale {
            std::fs::remove_file(&path)
                .map_err(|e| format!("removing `{}`: {e}", path.display()))?;
            removed.push(name.to_string());
        }
    }
    removed.sort();
    for file in files {
        let path = folder.join(&file.name);
        std::fs::write(&path, &file.text)
            .map_err(|e| format!("writing `{}`: {e}", path.display()))?;
    }
    Ok(removed)
}

/// The world in `world`, a WIT package with one world, resolved against the
/// packages written in `packages`, a folder each, whose interfaces the world
/// may import: the folders of a WIT folder's `deps/`, or those a package's
/// repository keeps its files in.
pub fn resolve(world: &str, packages: &[PathBuf]) -> Result<(Resolve, WorldId), String> {
    let mut resolve = Resolve {
        // WASI's time zone is `@unstable(feature = clocks-timezone)`.
        all_features: true,
        ..Default::default()
    };
    let deps = packages
        .iter()
        .map(|folder| {
            let mut map = wit_parser::SourceMap::new();
            map.push_dir(folder)
                .map_err(|e| format!("reading the WIT in `{}`: {e:#}", folder.display()))?;
            map.parse()
                .map_err(|(_, e)| format!("parsing the WIT in `{}`: {e}", folder.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let main = parse(".", &[("world.wit", world)])?;
    let package = resolve
        .push_groups(main, deps)
        .map_err(|e| format!("resolving the world: {e:#}"))?;
    let world = resolve
        .select_world(&[package], None)
        .map_err(|e| format!("selecting the world: {e:#}"))?;
    Ok((resolve, world))
}

/// The world rk-bindgen generates for the WIT folder `dir`, laid out as
/// WIT's tools lay one out: the folder's own package, its files at the top,
/// and in `deps/` a package for each it uses, a folder, a `.wit` file or a
/// binary `.wasm` package. `world` names a world of the folder's own
/// package, and without it the package's one world is taken.
/// A package that declares no world needs none written for it: the world
/// imports every interface the package declares, or, when it declares none
/// or the folder has no package of its own, every interface of each package
/// in `deps/`. A host's package dropped into `wit/` is generated as it is.
pub fn resolve_folder(dir: &Path, world: Option<&str>) -> Result<(Resolve, WorldId), String> {
    let mut resolve = Resolve {
        // A feature a package has not released yet is generated too, as
        // WASI's time zone is.
        all_features: true,
        ..Default::default()
    };
    let own = own_package(dir)?;
    if world.is_some() || own.as_ref().is_some_and(|own| own.main.worlds.len() > 0) {
        let (package, _) = resolve
            .push_dir(dir)
            .map_err(|e| format!("reading the WIT in `{}`: {e:#}", dir.display()))?;
        let world = resolve
            .select_world(&[package], world)
            .map_err(|e| format!("selecting the world: {e:#}"))?;
        return Ok((resolve, world));
    }
    let (deps, binaries) = deps_packages(dir, &mut resolve)?;
    let mut imports = own.iter().flat_map(interfaces_of).collect::<Vec<_>>();
    if imports.is_empty() {
        imports = deps
            .iter()
            .flat_map(interfaces_of)
            .chain(
                binaries
                    .iter()
                    .flat_map(|package| package_interfaces(&resolve, *package)),
            )
            .collect();
    }
    if imports.is_empty() {
        return Err(format!(
            "`{}` holds no interface to generate: no package with one, at its top or in its deps/",
            dir.display()
        ));
    }
    let world: String = imports
        .iter()
        .map(|interface| format!("  import {interface};\n"))
        .collect();
    let main = parse(
        ".",
        &[(
            "world.wit",
            &format!("package local:bindgen;\n\nworld bindgen {{\n{world}}}\n"),
        )],
    )?;
    let package = resolve
        .push_groups(main, own.into_iter().chain(deps).collect())
        .map_err(|e| format!("reading the WIT in `{}`: {e:#}", dir.display()))?;
    let world = resolve
        .select_world(&[package], Some("bindgen"))
        .map_err(|e| format!("selecting the world: {e:#}"))?;
    Ok((resolve, world))
}

/// What a component exports, as a program imports it, read from the
/// component's binary, which carries its types: a world importing each
/// interface the component's world exports, after each interface the
/// exports take types from. `bytes` is the component a library carries in
/// its `component/` folder; what the component itself imports is its own
/// business, not the program's.
pub fn component_exports(bytes: &[u8]) -> Result<(Resolve, WorldId), String> {
    let wit_parser::decoding::DecodedWasm::Component(mut resolve, world) =
        wit_parser::decoding::decode(bytes).map_err(|e| format!("reading the component: {e:#}"))?
    else {
        return Err(
            "a WIT package, not a component: a library carries the `.wasm` its language built"
                .to_string(),
        );
    };
    let component = &resolve.worlds[world];
    let mut exported = Vec::new();
    for item in component.exports.values() {
        match item {
            WorldItem::Interface { id, .. } => exported.push(*id),
            WorldItem::Function(function) => {
                return Err(format!(
                    "the component exports `{}` itself: rk imports a function from an interface",
                    function.name
                ));
            }
            WorldItem::Type { .. } => {}
        }
    }
    if exported.is_empty() {
        return Err("the component exports no interface".to_string());
    }
    // The interfaces the exports take types from, and those theirs take
    // from: the world imports them, as a component's world imports what its
    // exports use.
    let mut needed: Vec<InterfaceId> = Vec::new();
    let mut todo = exported.clone();
    while let Some(id) = todo.pop() {
        for dep in resolve.interface_direct_deps(id) {
            if !needed.contains(&dep) && !exported.contains(&dep) {
                needed.push(dep);
                todo.push(dep);
            }
        }
    }
    let imported =
        |item: &WorldItem| matches!(item, WorldItem::Interface { id, .. } if needed.contains(id));
    if let Some(missing) = needed.iter().find(|dep| {
        !component
            .imports
            .values()
            .any(|item| matches!(item, WorldItem::Interface { id, .. } if id == *dep))
    }) {
        return Err(format!(
            "the component's world does not import `{}`, whose types its exports take",
            resolve.id_of(*missing).unwrap_or_default()
        ));
    }
    let mut program = component.clone();
    program.name = "bindgen".to_string();
    program.imports = component
        .imports
        .iter()
        .filter(|(_, item)| imported(item))
        .chain(
            component
                .exports
                .iter()
                .filter(|(_, item)| matches!(item, WorldItem::Interface { .. })),
        )
        .map(|(key, item)| (key.clone(), item.clone()))
        .collect();
    program.exports = Default::default();
    let world = resolve.worlds.alloc(program);
    Ok((resolve, world))
}

/// The package written in the `.wit` files at the top of `dir`, or `None`
/// when there are none: the folder ships only what its `deps/` holds.
fn own_package(dir: &Path) -> Result<Option<wit_parser::UnresolvedPackageGroup>, String> {
    let has_wit = std::fs::read_dir(dir)
        .map_err(|e| format!("reading the WIT in `{}`: {e}", dir.display()))?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .any(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "wit"));
    if !has_wit {
        return Ok(None);
    }
    let mut map = wit_parser::SourceMap::new();
    map.push_dir(dir)
        .map_err(|e| format!("reading the WIT in `{}`: {e:#}", dir.display()))?;
    map.parse()
        .map(Some)
        .map_err(|(_, e)| format!("parsing the WIT in `{}`: {e}", dir.display()))
}

/// The packages in the `deps/` of `dir`, as WIT's tools read them, in the
/// order of their names: a folder or a `.wit` file each, parsed, and a
/// binary `.wasm` package each, read into `resolve` with the packages it
/// takes from, before the text that may take from it.
fn deps_packages(
    dir: &Path,
    resolve: &mut Resolve,
) -> Result<
    (
        Vec<wit_parser::UnresolvedPackageGroup>,
        Vec<wit_parser::PackageId>,
    ),
    String,
> {
    let deps = dir.join("deps");
    let mut binaries = Vec::new();
    if !deps.is_dir() {
        return Ok((Vec::new(), binaries));
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&deps)
        .map_err(|e| format!("reading `{}`: {e}", deps.display()))?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .collect();
    entries.sort();
    let mut packages = Vec::new();
    for path in entries {
        let mut map = wit_parser::SourceMap::new();
        let read = match (path.is_dir(), path.extension().and_then(|ext| ext.to_str())) {
            (true, _) => map.push_dir(&path),
            (false, Some("wit")) => map.push_file(&path),
            (false, Some("wasm" | "wat")) => {
                let package = resolve
                    .push_file(&path)
                    .map_err(|e| format!("reading the WIT in `{}`: {e:#}", path.display()))?;
                binaries.push(package);
                continue;
            }
            _ => continue,
        };
        read.map_err(|e| format!("reading the WIT in `{}`: {e:#}", path.display()))?;
        packages.push(
            map.parse()
                .map_err(|(_, e)| format!("parsing the WIT in `{}`: {e}", path.display()))?,
        );
    }
    Ok((packages, binaries))
}

/// Each interface the package `package` of `resolve` declares, as a world
/// imports it, written as [`interfaces_of`] writes them.
fn package_interfaces(resolve: &Resolve, package: wit_parser::PackageId) -> Vec<String> {
    let name = &resolve.packages[package].name;
    let version = name
        .version
        .as_ref()
        .map_or(String::new(), |version| format!("@{version}"));
    resolve.packages[package]
        .interfaces
        .keys()
        .map(|interface| format!("%{}:%{}/%{interface}{version}", name.namespace, name.name))
        .collect()
}

/// Each interface the package `group` declares, as a world imports it:
/// every name after a `%`, which WIT takes before any name, so that one
/// spelled like a keyword needs no list of them.
fn interfaces_of(group: &wit_parser::UnresolvedPackageGroup) -> Vec<String> {
    let name = &group.main.name;
    let version = name
        .version
        .as_ref()
        .map_or(String::new(), |version| format!("@{version}"));
    group
        .main
        .interfaces
        .iter()
        // An interface the package only uses from another has no name.
        .filter_map(|(_, interface)| interface.name.as_ref())
        .map(|interface| format!("%{}:%{}/%{interface}{version}", name.namespace, name.name))
        .collect()
}

/// Which programs reach the declarations rk-bindgen writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Reach {
    /// Only the library that holds them: their root namespace is INTERNAL,
    /// and the library's own blocks wrap them, as Std's wrap WASI's.
    #[default]
    Library,
    /// Every program that reads the library.
    Everyone,
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

/// The ST declarations of every interface `world` imports, in the order it
/// imports them, then the built-ins of the streams and the futures their
/// functions take or give, which only the library that holds them reaches.
pub fn generate(resolve: &Resolve, world: WorldId) -> Result<Vec<File>, String> {
    generate_reaching(resolve, world, Reach::Library)
}

/// [`generate`], the declarations reaching what `reach` says.
pub fn generate_reaching(
    resolve: &Resolve,
    world: WorldId,
    reach: Reach,
) -> Result<Vec<File>, String> {
    let mut sizes = SizeAlign::default();
    sizes
        .fill(resolve)
        .map_err(|e| format!("laying out the world's types: {e:#}"))?;
    let mut generator = Generator {
        resolve,
        sizes,
        reach,
        files: Vec::new(),
        namespaces: HashMap::new(),
        named: HashMap::new(),
    };
    for item in resolve.worlds[world].imports.values() {
        match item {
            WorldItem::Interface { id, .. } => generator.interface(*id)?,
            WorldItem::Function(function) => {
                return Err(format!(
                    "the world imports `{}` itself: rk imports a function from an interface",
                    function.name
                ));
            }
            _ => {}
        }
    }
    Ok(generator.files)
}

/// The text of a file under way, indented as deep as its namespaces.
#[derive(Default)]
pub(crate) struct Out {
    text: String,
    pub(crate) depth: usize,
}

impl Out {
    pub(crate) fn line(&mut self, line: &str) {
        if !line.is_empty() {
            for _ in 0..self.depth {
                self.text.push('\t');
            }
            self.text.push_str(line);
        }
        self.text.push('\n');
    }

    /// `docs` as a comment, its lines indented under the first's text.
    fn docs(&mut self, docs: &Docs) {
        if let Some(text) = docs.contents.as_deref() {
            self.comment(text);
        }
    }

    /// `text`, a sentence of the generator's own, as a comment, its lines
    /// broken between words.
    fn prose(&mut self, text: &str) {
        self.comment(&wrap(text, WIDTH, ""));
    }

    fn comment(&mut self, text: &str) {
        let text = text.trim().replace("/*", "/ *").replace("*/", "* /");
        let lines: Vec<&str> = text.lines().collect();
        let last = lines.len().saturating_sub(1);
        for (at, line) in lines.iter().enumerate() {
            let open = if at == 0 { "/* " } else { "   " };
            let close = if at == last { " */" } else { "" };
            let line = line.trim_end();
            match (at == 0, line.is_empty()) {
                (false, true) if at != last => self.line(""),
                _ => self.line(&format!("{open}{line}{close}")),
            }
        }
    }

    fn vars(&mut self, section: &str, vars: &[Var]) {
        if vars.is_empty() {
            return;
        }
        self.line(section);
        self.depth += 1;
        for var in vars {
            self.declaration(var);
        }
        self.depth -= 1;
        self.line("END_VAR");
    }

    /// A section whose variables each have their documentation on the lines
    /// above them, which the editor shows on hover.
    fn documented_vars(&mut self, section: &str, vars: &[(Var, String)]) {
        if vars.is_empty() {
            return;
        }
        self.line(section);
        self.depth += 1;
        for (var, text) in vars {
            self.prose(text);
            self.declaration(var);
        }
        self.depth -= 1;
        self.line("END_VAR");
    }

    fn declaration(&mut self, var: &Var) {
        match var.start {
            Some(start) => self.line(&format!("{} : {} := {start};", var.name, var.ty)),
            None => self.line(&format!("{} : {};", var.name, var.ty)),
        }
    }
}

/// Whether a set of built-ins serves a stream or a future.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Carrier {
    Stream,
    Future,
}

impl Carrier {
    fn prefix(self) -> &'static str {
        match self {
            Carrier::Stream => "stream",
            Carrier::Future => "future",
        }
    }
}

/// An interface's output under way, which its own built-ins join: the text,
/// what it leaves out, the names its namespace holds, its sets by type,
/// whether its code converts what it reads, and the interface, for the
/// types it names.
struct Here<'a> {
    out: &'a mut Out,
    skipped: &'a mut Vec<Skipped>,
    taken: &'a mut HashMap<String, String>,
    sets: &'a mut Vec<(Carrier, String)>,
    /// The blocks each set of the interface manages its ends with.
    managers: &'a mut HashMap<(Carrier, String), Managers>,
    /// Set when a built-in's value is read out of words.
    converts: &'a mut bool,
    owner: Option<TypeOwner>,
}

struct Generator<'a> {
    resolve: &'a Resolve,
    /// Where the canonical ABI puts each type's values.
    sizes: SizeAlign,
    reach: Reach,
    files: Vec<File>,
    /// The interface each namespace written so far is, by the namespace
    /// in capitals, and the one each file is, by its name in lowercase.
    namespaces: HashMap<String, String>,
    named: HashMap<String, String>,
}

/// What a function's result is declared as.
enum Returned {
    /// The FUNCTION's own value, or an output per field.
    Fields(Shape),
    /// Words the generated code reads its values out of.
    Words(lift::Lifted),
    /// A value of a STRUCT the canonical ABI lays out as a record of
    /// scalars: the import gives its values as its outputs, `raw`, as it
    /// gives a record's, and the FUNCTION puts each in its member, at the
    /// same place in `targets`.
    Members {
        lifted: lift::Lifted,
        raw: Vec<Var>,
        targets: Vec<String>,
    },
}

impl Returned {
    /// The outputs a call names.
    fn outputs(&self) -> &[Var] {
        match self {
            Returned::Words(lifted) | Returned::Members { lifted, .. } => &lifted.outputs,
            Returned::Fields(Shape::Outputs(outputs)) => outputs,
            Returned::Fields(_) => &[],
        }
    }

    /// What a call passes for the string or the list the result holds: a
    /// STRING, or an array per field of the list's elements.
    fn places(&self) -> &[lift::Place] {
        match self {
            Returned::Words(lifted) => &lifted.places,
            Returned::Fields(_) | Returned::Members { .. } => &[],
        }
    }

    /// The type of the FUNCTION's own value, when it has one: a scalar, or
    /// a value of a STRUCT the generated code reads.
    fn value(&self) -> Option<&str> {
        match self {
            Returned::Fields(Shape::Return(st)) => Some(st),
            Returned::Words(lifted) | Returned::Members { lifted, .. } => lifted.returns.as_deref(),
            Returned::Fields(_) => None,
        }
    }
}

/// What the type `def` is declared as in ST, where it is: an alias of a
/// scalar or of a STRUCT that is no `use`, an enum, a variant, flags, a
/// record, a tuple WIT names.
fn declared(
    resolve: &Resolve,
    def: &wit_parser::TypeDef,
    here: Option<TypeOwner>,
) -> Option<&'static str> {
    match &def.kind {
        TypeDefKind::Type(Type::Id(other)) if resolve.types[*other].owner != def.owner => None,
        TypeDefKind::Type(inner) => (shapes::scalar(resolve, *inner, here).is_some()
            || types::structured(resolve, *inner).is_some())
        .then_some("type"),
        TypeDefKind::Enum(_) => Some("enum"),
        TypeDefKind::Variant(_) => Some("variant"),
        TypeDefKind::Flags(flags) => shapes::flags_base(flags).map(|_| "flags"),
        TypeDefKind::Record(_) => Some("record"),
        TypeDefKind::Tuple(_) => Some("tuple"),
        TypeDefKind::Resource => Some("resource"),
        _ => None,
    }
}

/// The STRUCT `name` of `members`, each after what WIT says of it.
fn structure(out: &mut Out, name: &str, members: &[types::Member]) {
    out.line(&format!("TYPE {name} : STRUCT"));
    out.depth += 1;
    for member in members {
        if let Some(docs) = &member.docs {
            out.comment(docs);
        }
        out.line(&format!("{} : {};", member.name, member.ty));
    }
    out.depth -= 1;
    out.line("END_STRUCT END_TYPE");
    out.line("");
}

/// The typed enum `name` on `base`, each of its `values` written `NAME :=
/// number`, starting at its first value or at `start`.
fn named_values(out: &mut Out, name: &str, base: &str, values: &[String], start: Option<&str>) {
    out.line(&format!("TYPE {name} : {base} ("));
    out.depth += 1;
    let last = values.len().saturating_sub(1);
    for (at, value) in values.iter().enumerate() {
        out.line(&format!("{value}{}", if at == last { "" } else { "," }));
    }
    out.depth -= 1;
    match start {
        Some(start) => out.line(&format!(") := {start}; END_TYPE")),
        None => out.line("); END_TYPE"),
    }
    out.line("");
}

/// The unsigned integer a case is held in.
fn tag_base(tag: Int) -> &'static str {
    match tag {
        Int::U8 => "USINT",
        Int::U16 => "UINT",
        Int::U32 => "UDINT",
        Int::U64 => "ULINT",
    }
}

impl Generator<'_> {
    fn interface(&mut self, id: InterfaceId) -> Result<(), String> {
        let resolve = self.resolve;
        let interface = &resolve.interfaces[id];
        let module = resolve
            .id_of(id)
            .ok_or("an interface without a name cannot be imported")?;
        let package = interface
            .package
            .map(|package| &resolve.packages[package].name)
            .ok_or("an interface outside a package cannot be imported")?;
        let interface_name = interface.name.as_deref().unwrap_or_default();
        let here = Some(TypeOwner::Interface(id));
        // The root is the package's namespace, which a type another
        // interface declares is named from too ([`shapes::qualified`]).
        let root = names::namespace(&package.namespace);
        let namespace = format!(
            "{root}.{}.{}",
            names::namespace(&package.name),
            names::namespace(interface_name)
        );
        let file = format!("{}-{}-{interface_name}.st", package.namespace, package.name);
        if let Some(other) = self
            .namespaces
            .insert(namespace.to_ascii_uppercase(), module.clone())
        {
            return Err(format!(
                "`{other}` and `{module}` would both be the namespace `{namespace}`, which ST takes for one"
            ));
        }
        if let Some(other) = self.named.insert(file.to_ascii_lowercase(), module.clone()) {
            return Err(format!(
                "`{other}` and `{module}` would both be the file `{file}`"
            ));
        }

        let mut out = Out {
            text: String::new(),
            depth: 3,
        };
        let mut skipped = Vec::new();
        // Each ST name the interface's namespace holds, by what it names:
        // the names WIT gives first, each type's, then each function's.
        let mut taken: HashMap<String, String> = HashMap::new();
        // The types' names, which a function takes a `_` after.
        let mut type_names: Vec<String> = Vec::new();
        for (name, &ty) in &interface.types {
            if let Some(kind) = declared(resolve, &resolve.types[ty], here) {
                let st_name = names::upper(name);
                let what = format!("the {kind} `{name}`");
                let mut st_names = vec![(st_name, what.clone())];
                // A variant whose cases hold values names its cases apart.
                if let TypeDefKind::Variant(variant) = &resolve.types[ty].kind
                    && shapes::carries(variant)
                {
                    st_names.push((
                        names::upper(&format!("{name}-kind")),
                        format!("the cases of `{name}`"),
                    ));
                }
                for (st_name, what) in st_names {
                    if let Err(other) = claim(&mut taken, &st_name, &what) {
                        return Err(format!(
                            "{other} and {what} would both be `{st_name}` in `{module}`"
                        ));
                    }
                    type_names.push(st_name);
                }
            }
        }
        // A function ST takes for an earlier one, or for what the generator
        // makes. One named as a type takes a `_` after its name: a program
        // names a type in its declarations, and WIT names both.
        let mut met: HashMap<&str, String> = HashMap::new();
        let mut pous: HashMap<&str, String> = HashMap::new();
        for function in interface.functions.values() {
            let Some(pou) = pou_name(resolve, function) else {
                continue;
            };
            let pou = match type_names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&pou))
            {
                true => format!("{pou}_"),
                false => pou,
            };
            match claim(&mut taken, &pou, &format!("`{}`", function.name)) {
                Ok(()) => {
                    pous.insert(function.name.as_str(), pou);
                }
                Err(other) => {
                    met.insert(
                        function.name.as_str(),
                        format!("it would be `{pou}`, which {other} is already"),
                    );
                }
            }
        }
        for (name, &ty) in &interface.types {
            let def = &resolve.types[ty];
            match &def.kind {
                // A `use`: the generated code names the type where it is defined.
                TypeDefKind::Type(Type::Id(other)) if resolve.types[*other].owner != def.owner => {}
                TypeDefKind::Type(inner) => {
                    match shapes::scalar(resolve, *inner, here).or_else(|| {
                        types::structured(resolve, *inner)
                            .filter(|&id| types::declared(resolve, id, here).is_ok())
                            .map(|id| shapes::type_name(resolve, id, here))
                    }) {
                        Some(st) => {
                            let st_name = names::upper(name);
                            out.docs(&def.docs);
                            out.line(&format!("TYPE {st_name} : {st}; END_TYPE"));
                            out.line("");
                        }
                        // A type whose STRUCT is left out, which says why.
                        None if let Some(id) = types::structured(resolve, *inner) => {
                            skipped.push(Skipped {
                                item: format!("`{name}`"),
                                reason: format!(
                                    "it names `{}`, which is left out",
                                    types::wit_name(resolve, id)
                                ),
                            })
                        }
                        // A string, an option or a result: the functions that
                        // take or give it say what they can.
                        None if passed(resolve, *inner) => {}
                        None => skipped.push(Skipped {
                            item: format!("`{name}`"),
                            reason: format!(
                                "it names {}, which ST has no type for yet",
                                shapes::what(resolve, *inner)
                            ),
                        }),
                    }
                }
                TypeDefKind::Enum(enumeration) => {
                    let cases: Vec<String> = enumeration
                        .cases
                        .iter()
                        .enumerate()
                        .map(|(at, case)| format!("{} := {at}", names::upper(&case.name)))
                        .collect();
                    let st_name = names::upper(name);
                    out.docs(&def.docs);
                    named_values(
                        &mut out,
                        &st_name,
                        tag_base(enumeration.tag()),
                        &cases,
                        None,
                    );
                }
                // A variant's cases, by name; one whose cases hold values
                // is a STRUCT of its case, `kind`, and of what each case
                // holds.
                TypeDefKind::Variant(variant) => {
                    let cases: Vec<String> = variant
                        .cases
                        .iter()
                        .enumerate()
                        .map(|(at, case)| format!("{} := {at}", names::upper(&case.name)))
                        .collect();
                    let st_name = names::upper(name);
                    match shapes::carries(variant) {
                        false => {
                            out.docs(&def.docs);
                            named_values(&mut out, &st_name, tag_base(variant.tag()), &cases, None);
                        }
                        true => match types::members(resolve, ty, here) {
                            Ok(members) => {
                                let kind = names::upper(&format!("{name}-kind"));
                                out.prose(&format!("The cases of `{name}`."));
                                named_values(
                                    &mut out,
                                    &kind,
                                    tag_base(variant.tag()),
                                    &cases,
                                    None,
                                );
                                out.docs(&def.docs);
                                structure(&mut out, &st_name, &members);
                            }
                            Err(reason) => skipped.push(Skipped {
                                item: format!("`{name}`"),
                                reason,
                            }),
                        },
                    }
                    for case in &variant.cases {
                        let Some(held) = case.ty else { continue };
                        // What no call gives back, or else a text, which
                        // comes back from a call that waits for it, where
                        // it is the one block of the result.
                        let unread =
                            |texts| lift::unreadable(resolve, &self.sizes, held, here, texts);
                        let reason = match (unread(true), unread(false)) {
                            (Some(what), _) => {
                                Some(format!("{what}, which an import cannot give back yet"))
                            }
                            (None, Some(what)) => {
                                Some(format!("{what}, which an async call cannot give back yet"))
                            }
                            (None, None) => None,
                        };
                        if let Some(reason) = reason {
                            skipped.push(Skipped {
                                item: format!("what `{}` of `{name}` holds", case.name),
                                reason,
                            });
                        }
                    }
                }
                // Each flag by name, a bit of the bit string that holds them.
                TypeDefKind::Flags(flags) if shapes::flags_base(flags).is_some() => {
                    let (base, bytes) = shapes::flags_base(flags).unwrap_or(("DWORD", 4));
                    let digits = 2 * bytes;
                    let values: Vec<String> = flags
                        .flags
                        .iter()
                        .enumerate()
                        .map(|(at, flag)| {
                            format!(
                                "{} := 16#{:0digits$X}",
                                names::upper(&flag.name),
                                1u32 << at
                            )
                        })
                        .collect();
                    let st_name = names::upper(name);
                    out.docs(&def.docs);
                    // No flag set: a value of flags starts at zero, and not
                    // at its first flag, as a value of an enum starts at
                    // its first value.
                    named_values(&mut out, &st_name, base, &values, Some("0"));
                }
                // A handle of the resource: its own name for a UDINT, which
                // says what a declaration takes and passes as any UDINT does.
                TypeDefKind::Resource => {
                    let st_name = names::upper(name);
                    out.docs(&def.docs);
                    out.line(&format!("TYPE {st_name} : UDINT; END_TYPE"));
                    out.line("");
                    let drop = own(
                        &mut taken,
                        names::upper(&format!("{name}-drop")),
                        &format!("the drop of `{name}`"),
                    );
                    out.prose(&format!(
                        "Lets go of `handle`, a handle of `{name}` the host gave."
                    ));
                    out.line(&format!("{{extern '{module}' '[resource-drop]{name}'}}"));
                    out.line(&format!("FUNCTION {drop}"));
                    out.vars(
                        "VAR_INPUT",
                        &[Var {
                            name: "handle".to_string(),
                            ty: st_name,
                            flat: 1,
                            start: None,
                        }],
                    );
                    out.line("END_FUNCTION");
                    out.line("");
                }
                // A STRUCT of its fields, or of its items.
                TypeDefKind::Record(_) | TypeDefKind::Tuple(_) => {
                    match types::members(resolve, ty, here) {
                        Ok(members) => {
                            out.docs(&def.docs);
                            structure(&mut out, &names::upper(name), &members);
                        }
                        Err(reason) => skipped.push(Skipped {
                            item: format!("`{name}`"),
                            reason,
                        }),
                    }
                }
                // Passed value by value, or a handle, which is a UDINT.
                TypeDefKind::Option(_)
                | TypeDefKind::Result(_)
                | TypeDefKind::Handle(_)
                | TypeDefKind::Future(_)
                | TypeDefKind::Stream(_) => {}
                _ => skipped.push(Skipped {
                    item: format!("`{name}`"),
                    reason: format!(
                        "it is {}, which ST has no type for yet",
                        shapes::what(resolve, Type::Id(ty))
                    ),
                }),
            }
        }
        // The sets of built-ins of this interface's own types, by type, and
        // the blocks each manages its ends with.
        let mut own_sets: Vec<(Carrier, String)> = Vec::new();
        let mut managers: HashMap<(Carrier, String), Managers> = HashMap::new();
        // The FUNCTIONs that are their import as it is, which a writer may
        // call.
        let mut plain: Vec<(&Function, Declared)> = Vec::new();
        // Whether a result is read out of words, with `Std.Convert`.
        let mut converts = false;
        for function in interface.functions.values() {
            let written = match met.remove(function.name.as_str()) {
                Some(reason) => Err(reason),
                None => self.function(
                    &mut out,
                    &module,
                    function,
                    pous.get(function.name.as_str()),
                    here,
                    &mut taken,
                ),
            };
            match written {
                Ok((converted, declared)) => {
                    converts |= converted;
                    plain.extend(declared.map(|declared| (function, declared)));
                }
                Err(reason) => skipped.push(Skipped {
                    item: format!("`{}`", function.name),
                    reason,
                }),
            }
            let mut here_out = Here {
                out: &mut out,
                skipped: &mut skipped,
                taken: &mut taken,
                sets: &mut own_sets,
                managers: &mut managers,
                converts: &mut converts,
                owner: here,
            };
            self.built_ins(&module, function, &mut here_out);
        }
        // Each function that takes a stream of bytes from the program, made,
        // written and ended by a block of its own.
        for (function, declared) in &plain {
            let blocks = Blocks {
                resolve,
                managers: &managers,
                namespace: &namespace,
            };
            for form in [Form::Text, Form::Array] {
                function_writer(&blocks, &mut out, &mut taken, function, declared, form);
                function_reader(&blocks, &mut out, &mut taken, function, declared, form);
            }
        }

        let mut text = header(&[package.to_string()]);
        if !out.text.is_empty() {
            let mut wrapper = Out::default();
            wrapper.line(&match self.reach {
                Reach::Library => format!("NAMESPACE INTERNAL {root}"),
                Reach::Everyone => format!("NAMESPACE {root}"),
            });
            wrapper.depth += 1;
            wrapper.line(&format!("NAMESPACE {}", names::namespace(&package.name)));
            wrapper.depth += 1;
            wrapper.docs(&interface.docs);
            wrapper.line(&format!("NAMESPACE {}", names::namespace(interface_name)));
            if converts {
                wrapper.depth += 1;
                wrapper.line("USING Std.Convert;");
                wrapper.line("");
            }
            text.push_str(&wrapper.text);
            text.push_str(out.text.trim_end_matches('\n'));
            text.push('\n');
            let mut closing = Out {
                text: String::new(),
                depth: 2,
            };
            closing.line("END_NAMESPACE");
            closing.depth -= 1;
            closing.line("END_NAMESPACE");
            closing.depth -= 1;
            closing.line("END_NAMESPACE");
            text.push_str(&closing.text);
        }
        text.push_str(&left_out(&skipped));
        self.files.push(File {
            name: file,
            text,
            skipped,
        });
        Ok(())
    }

    /// The declaration of `function`, imported from `module`, written to
    /// `out` under the name `pou` that `taken` holds for it (`None` for a
    /// property), and whether its code converts a value, with
    /// `Std.Convert`. Its import may be apart from it: `<NAME>_RAW`, whose
    /// parameters `<NAME>` packs or passes field by field and whose result
    /// it reads, which `taken` then holds too. `Err` says why it cannot be.
    fn function(
        &self,
        out: &mut Out,
        module: &str,
        function: &Function,
        pou: Option<&String>,
        here: Option<TypeOwner>,
        taken: &mut HashMap<String, String>,
    ) -> Result<(bool, Option<Declared>), String> {
        let resolve = self.resolve;
        let Some(pou) = pou.cloned() else {
            return Err("it is a property, which rk does not import".to_string());
        };
        let asynchronous = function.kind.is_async();
        let signature = resolve.wasm_signature(
            match asynchronous {
                true => AbiVariant::GuestImportAsync,
                false => AbiVariant::GuestImport,
            },
            function,
        );
        let raw = spare(taken, format!("{pou}_RAW"));
        let words_back = if asynchronous { 1 } else { 2 };
        // A variable WIT names keeps away from the names the code has
        // itself, which are known once it is laid out: laid out again
        // where one meets.
        let mut scope = Scope::default();
        let (lowered, returned) = loop {
            let lowered = lower::lower(
                resolve,
                &self.sizes,
                function,
                signature.indirect_params,
                here,
                &scope,
            )?;
            if !asynchronous && signature.indirect_params {
                return Err(format!(
                    "it takes {} values, which the canonical ABI passes in memory past {MAX_FLAT_PARAMS}",
                    lowered.flat
                ));
            }
            let cannot = |what: &str| {
                format!("its result holds {what}, which an import cannot give back yet")
            };
            let direct = shapes::result(resolve, function.result, asynchronous, here, &scope);
            // A value of a STRUCT laid out as a record of scalars comes back
            // in the import's outputs, as a record does.
            let scalars = match (&direct, function.result) {
                (Err(_), Some(ty)) => lift::record_of_scalars(resolve, ty, here),
                _ => None,
            };
            let returned = match (direct, function.result, scalars) {
                (Ok(shape), _, _) => Returned::Fields(shape),
                (Err(_), Some(ty), Some(scalars)) => {
                    let lifted = lift::lift(
                        resolve,
                        &self.sizes,
                        ty,
                        here,
                        &scope,
                        false,
                        (!asynchronous).then_some(pou.as_str()),
                    )
                    .map_err(cannot)?;
                    // The import's outputs keep clear of the variables of
                    // the import and of its POU: a parameter's, a block's
                    // output or its `DONE`.
                    let mut held: Vec<String> = lowered
                        .params
                        .iter()
                        .chain(&lowered.inputs)
                        .chain(&lifted.outputs)
                        .map(|var| var.name.clone())
                        .collect();
                    if asynchronous {
                        held.push("DONE".to_string());
                    }
                    let (raw, targets) = lifted.as_outputs(scalars, held).into_iter().unzip();
                    Returned::Members {
                        lifted,
                        raw,
                        targets,
                    }
                }
                // What no record of outputs is laid out as is read out of
                // the words it comes back in, where the canonical ABI
                // writes it in memory: an async call's result always,
                // another's past one value.
                // A string comes back to a call that waits for it: the
                // host writes it into the STRING the call passes.
                (Err(_), Some(ty), None) if asynchronous || signature.retptr => Returned::Words(
                    lift::lift(
                        resolve,
                        &self.sizes,
                        ty,
                        here,
                        &scope,
                        !asynchronous,
                        (!asynchronous).then_some(pou.as_str()),
                    )
                    .map_err(cannot)?,
                ),
                // One value, inside tuples that hold it alone: the canonical
                // ABI returns it as it is, and the import gives it as the
                // one output it would be read into.
                (Err(what), Some(ty), None) => {
                    match lift::lift(resolve, &self.sizes, ty, here, &scope, false, None)
                        .map_err(cannot)?
                    {
                        lifted if lifted.outputs.len() == 1 && !lifted.structured => {
                            Returned::Fields(Shape::Outputs(lifted.outputs))
                        }
                        _ => return Err(cannot(what)),
                    }
                }
                (Err(what), None, _) => return Err(cannot(what)),
            };
            let apart = lowered.apart()
                || matches!(returned, Returned::Words(_) | Returned::Members { .. });
            let mut own = Vec::new();
            // A FUNCTION's value has its name, which no variable of it can.
            if returned.value().is_some() {
                own.push(pou.clone());
                if apart {
                    own.push(raw.clone());
                }
            }
            if asynchronous {
                own.push("DONE".to_string());
            }
            // A list's count is the generated code's name for it.
            if lowered.lists() {
                own.push(lower::LIST_COUNT.to_string());
            }
            if apart {
                if asynchronous {
                    own.push("call".to_string());
                }
                own.extend(lowered.words().into_iter().map(|var| var.name));
                match &returned {
                    Returned::Words(lifted) => {
                        own.extend(lifted.words(words_back).into_iter().map(|var| var.name))
                    }
                    Returned::Members { .. } | Returned::Fields(_) => {}
                }
            }
            let named = || {
                lowered
                    .inputs
                    .iter()
                    .chain(returned.outputs())
                    .chain(returned.places().iter().map(|place| &place.var))
            };
            own.retain(|name| {
                !scope.holds(name) && named().any(|var| var.name.eq_ignore_ascii_case(name))
            });
            if own.is_empty() {
                break (lowered, returned);
            }
            own.iter().for_each(|name| scope.hold(name));
        };
        // What the import gives back: the words a result is read out of, a
        // call that is not async returning a record in memory from two
        // fields on, or the outputs as they are.
        let outputs = returned.outputs();
        let given: Vec<Var> = match &returned {
            Returned::Words(lifted) => lifted.words(words_back),
            Returned::Members { raw, .. } => raw.clone(),
            Returned::Fields(_) => outputs.to_vec(),
        };
        let apart =
            lowered.apart() || matches!(returned, Returned::Words(_) | Returned::Members { .. });
        // The STRING the host writes the result's string into: the
        // caller's, which the import's wrapper passes on, or a member of
        // the STRUCT the FUNCTION gives back; the arrays of its list; or
        // the STRING of its text, an output of the import, cut to fit.
        let places: Vec<lift::Place> = returned.places().to_vec();
        let text: Vec<Var> = places.iter().map(|place| place.var.clone()).collect();
        let (in_outs, texts): (Vec<Var>, Vec<Var>) = {
            let (cut, kept): (Vec<&lift::Place>, Vec<&lift::Place>) =
                places.iter().partition(|place| place.cut);
            let vars = |places: Vec<&lift::Place>| {
                places.into_iter().map(|place| place.var.clone()).collect()
            };
            (vars(kept), vars(cut))
        };
        // What no member holds: the caller's STRING or arrays, and a text,
        // which the FUNCTION gives as an output.
        let own = |cut: bool| -> Vec<Var> {
            places
                .iter()
                .filter(|place| place.source.is_none() && place.cut == cut)
                .map(|place| place.var.clone())
                .collect()
        };
        let (own_text, own_texts) = (own(false), own(true));
        let mut variables: Vec<&str> = lowered.inputs.iter().map(|var| var.name.as_str()).collect();
        variables.extend(lowered.sources.iter().map(|(param, _)| param.as_str()));
        if asynchronous {
            variables.push("DONE");
        }
        variables.extend(outputs.iter().map(|var| var.name.as_str()));
        variables.extend(text.iter().map(|var| var.name.as_str()));
        let words = lowered.words();
        if apart {
            if asynchronous {
                variables.push("call");
            }
            variables.extend(words.iter().map(|var| var.name.as_str()));
            if let Returned::Words(_) | Returned::Members { .. } = returned {
                variables.extend(given.iter().map(|var| var.name.as_str()));
            }
        }
        for (at, variable) in variables.iter().enumerate() {
            if variables[..at]
                .iter()
                .any(|earlier| earlier.eq_ignore_ascii_case(variable))
            {
                return Err(format!(
                    "two of its variables would be `{variable}`, which ST takes for one"
                ));
            }
        }

        let name = &function.name;
        // The import's own value: a scalar it returns as it is.
        let value = match &returned {
            Returned::Fields(Shape::Return(st)) => format!(" : {st}"),
            _ => String::new(),
        };
        // The FUNCTION's: that scalar, or a STRUCT it reads.
        let pou_value = returned
            .value()
            .map(|st| format!(" : {st}"))
            .unwrap_or_default();
        let import = if apart {
            taken.insert(raw.to_ascii_uppercase(), format!("the import of `{name}`"));
            raw
        } else {
            pou.clone()
        };
        // What the FUNCTION does to the import's parameters: packs values
        // into words, or passes the fields of a STRUCT one by one.
        let packs = match (lowered.packs(), lowered.lists()) {
            (true, _) => "packs",
            (false, true) => "takes from the caller's array",
            (false, false) => "passes field by field",
        };
        // The values of a result the import gives as they are, and where
        // the POU puts them: in its own value, or in its output.
        let (values, members) = match (given.len(), outputs.first()) {
            (1, None) => ("the one value of its result", "its member".to_string()),
            (_, None) => ("the values of its result", "its members".to_string()),
            (1, Some(output)) => (
                "the one value of its result",
                format!("the member of `{}`", output.name),
            ),
            (_, Some(output)) => (
                "the values of its result",
                format!("the members of `{}`", output.name),
            ),
        };
        if apart {
            out.prose(&match (lowered.apart(), &returned) {
                (false, _) if !texts.is_empty() => format!(
                    "`{name}` as the host gives it: its result in words of 8 bytes, as the canonical ABI lays it out, which `{pou}` reads, and its text in `{}`, where the host writes it, cut to fit.",
                    texts[0].name
                ),
                (false, _) if text.len() == 1 && text[0].ty == "STRING" => format!(
                    "`{name}` as the host gives it: its result in words of 8 bytes, as the canonical ABI lays it out, which `{pou}` reads, and its string in `{}`, where the host writes it.",
                    text[0].name
                ),
                (false, _) if !text.is_empty() => format!(
                    "`{name}` as the host gives it: its result in words of 8 bytes, as the canonical ABI lays it out, which `{pou}` reads, and its list in {}, where the host writes it.",
                    text.iter()
                        .map(|var| format!("`{}`", var.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                (false, Returned::Members { .. }) => format!(
                    "`{name}` as the host gives it: {values}, which `{pou}` puts in {members}."
                ),
                (false, _) => format!(
                    "`{name}` as the host gives it: its result in words of 8 bytes, as the canonical ABI lays it out, which `{pou}` reads."
                ),
                (true, Returned::Fields(_)) => format!(
                    "`{name}` as the host takes it: its parameters as the canonical ABI passes them, which `{pou}` {packs}."
                ),
                (true, Returned::Members { .. }) => format!(
                    "`{name}` as the host has it: its parameters as the canonical ABI passes them, which `{pou}` {packs}, and {values}, which it puts in {members}."
                ),
                (true, Returned::Words(_)) => format!(
                    "`{name}` as the host has it: its parameters as the canonical ABI passes them, which `{pou}` {packs}, and its result in words of 8 bytes, which it reads."
                ),
            });
        } else {
            out.docs(&function.docs);
        }
        // The import: its parameters, and what it gives back.
        match asynchronous {
            false => {
                out.line(&format!("{{extern '{module}' '{name}'}}"));
                let private = if apart { "PRIVATE " } else { "" };
                out.line(&format!("FUNCTION {private}{import}{value}"));
                out.vars("VAR_INPUT", &lowered.params);
                out.vars("VAR_IN_OUT", &in_outs);
                out.vars("VAR_OUTPUT", &[given.clone(), texts.clone()].concat());
                out.line("END_FUNCTION");
            }
            true => {
                out.line(&format!("{{extern async '{module}' '{name}'}}"));
                out.line(&format!("FUNCTION_BLOCK {import}"));
                out.vars("VAR_INPUT", &lowered.params);
                out.vars("VAR_OUTPUT", &[vec![done()], given.clone()].concat());
                out.line("END_FUNCTION_BLOCK");
            }
        }
        out.line("");
        // What a writer calls: a FUNCTION that is its import as it is and
        // gives back no output, or the block of an `async func`, each
        // parameter an input of it, in order, and none in and out.
        let one_to_one = in_outs.is_empty()
            && lowered.in_outs.is_empty()
            && texts.is_empty()
            && lowered.inputs.len() == function.params.len();
        let declared = match (asynchronous, apart) {
            (false, false) if one_to_one => Some(Declared {
                pou: pou.clone(),
                inputs: lowered.inputs.clone(),
                block: None,
                given: given.clone(),
            }),
            (true, _) if one_to_one => Some(Declared {
                pou: pou.clone(),
                inputs: lowered.inputs.clone(),
                block: Some(outputs.to_vec()),
                given: Vec::new(),
            }),
            _ => None,
        };
        if !apart {
            return Ok((false, declared));
        }

        // What a call names: the values themselves, packed into the
        // import's parameters before its call and read out of what it
        // gives back after.
        let passed: Vec<String> = lowered
            .params
            .iter()
            .map(|var| format!("{} := {}", var.name, lowered.source(&var.name)))
            .collect();
        out.docs(&function.docs);
        match asynchronous {
            false => {
                out.line(&format!("FUNCTION {pou}{pou_value}"));
                out.vars("VAR_INPUT", &lowered.inputs);
                out.vars("VAR_IN_OUT", &[lowered.in_outs.clone(), own_text].concat());
                out.vars("VAR_OUTPUT", &[outputs.to_vec(), own_texts].concat());
                let mut locals = words;
                if let Returned::Words(_) = returned {
                    locals.extend(given.iter().cloned());
                }
                out.vars("VAR", &locals);
                out.depth += 1;
                lowered.write(out);
                // A STRING or arrays the call passes, a text it receives.
                let written = places.iter().map(|place| {
                    format!(
                        "{} {} {}",
                        place.var.name,
                        if place.cut { "=>" } else { ":=" },
                        place.source.as_deref().unwrap_or(&place.var.name)
                    )
                });
                let received = given.iter().enumerate().map(|(at, var)| match &returned {
                    Returned::Members { targets, .. } => format!("{} => {}", var.name, targets[at]),
                    _ => format!("{0} => {0}", var.name),
                });
                let arguments: Vec<String> =
                    passed.into_iter().chain(written).chain(received).collect();
                match value.is_empty() {
                    true => out.line(&format!("{import}({});", arguments.join(", "))),
                    false => out.line(&format!("{pou} := {import}({});", arguments.join(", "))),
                }
                if let Returned::Words(lifted) = &returned {
                    lifted.write(out, "w");
                }
                out.depth -= 1;
                out.line("END_FUNCTION");
            }
            true => {
                out.line(&format!("FUNCTION_BLOCK {pou}"));
                out.vars("VAR_INPUT", &lowered.inputs);
                out.vars("VAR_IN_OUT", &lowered.in_outs);
                out.vars("VAR_OUTPUT", &[vec![done()], outputs.to_vec()].concat());
                let mut locals = words;
                locals.push(Var {
                    name: "call".to_string(),
                    ty: import.clone(),
                    flat: 1,
                    start: None,
                });
                out.vars("VAR", &locals);
                out.depth += 1;
                lowered.write(out);
                out.line(&format!("call({});", passed.join(", ")));
                out.line("DONE := call.DONE;");
                if !outputs.is_empty() {
                    out.line("IF call.DONE THEN");
                    out.depth += 1;
                    match &returned {
                        Returned::Words(lifted) => lifted.write(out, "call.w"),
                        Returned::Members { raw, targets, .. } => {
                            for (raw, target) in raw.iter().zip(targets) {
                                out.line(&format!("{target} := call.{};", raw.name));
                            }
                        }
                        Returned::Fields(_) => {
                            for output in outputs {
                                out.line(&format!("{0} := call.{0};", output.name));
                            }
                        }
                    }
                    out.depth -= 1;
                    out.line("END_IF;");
                }
                out.depth -= 1;
                out.line("END_FUNCTION_BLOCK");
            }
        }
        out.line("");
        Ok((
            lowered.converts() || matches!(&returned, Returned::Words(lifted) if lifted.converts()),
            declared,
        ))
    }

    /// The built-ins of each stream and future `function` takes or gives
    /// whose type the interface has none for yet, imported from `module`
    /// under its name: they are the interface's own, in its namespace, as
    /// the import names them after its function. A stream of bytes made by
    /// one interface's built-ins is any other's too: a stream's type is its
    /// element's.
    fn built_ins(&mut self, module: &str, function: &Function, here: &mut Here<'_>) {
        let resolve = self.resolve;
        for (index, ty) in function
            .find_futures_and_streams(resolve)
            .into_iter()
            .enumerate()
        {
            let (carrier, payload) = match &resolve.types[ty].kind {
                TypeDefKind::Stream(payload) => (Carrier::Stream, *payload),
                TypeDefKind::Future(payload) => (Carrier::Future, *payload),
                _ => continue,
            };
            let key = (carrier, type_key(resolve, payload));
            if here.sets.contains(&key) {
                continue;
            }
            let (fed, given) = directions(resolve, here.owner, carrier, &key.1);
            here.sets.push(key.clone());
            // A set says what it is: `BYTES_STREAM`, `RESULT_FUTURE`.
            let name = format!(
                "{}_{}",
                set_name(resolve, carrier, payload),
                capitals(carrier.prefix())
            );
            let anchor = format!(
                "{module}' '[{}-{{}}-{index}]{}",
                carrier.prefix(),
                function.name
            );
            let (written, managers) = built_in_set(
                resolve,
                &self.sizes,
                here.out,
                here.skipped,
                here.converts,
                here.taken,
                carrier,
                &name,
                payload,
                &anchor,
                here.owner,
                fed,
                given,
            );
            let what = format!("a built-in of `{}`", wit_text(resolve, payload));
            for pou in written {
                here.taken.insert(pou.to_ascii_uppercase(), what.clone());
            }
            here.managers.insert(key, managers);
        }
    }
}

/// The outputs and the body of a block that gives back what `raw`, the
/// block of its import, called with `passed`, gives: `DONE` and the values
/// `lifted` reads out of the words of `raw`, or, where `copied` names
/// them, takes from the outputs of `raw` into their members. Each takes
/// its value in the call that sees the import's call finish.
fn reader(out: &mut Out, raw: &str, passed: &str, lifted: &lift::Lifted, copied: &[(Var, String)]) {
    out.vars(
        "VAR_OUTPUT",
        &[vec![done()], lifted.outputs.clone()].concat(),
    );
    out.vars(
        "VAR",
        &[Var {
            name: "call".to_string(),
            ty: raw.to_string(),
            flat: 1,
            start: None,
        }],
    );
    out.depth += 1;
    out.line(&format!("call({passed});"));
    out.line("DONE := call.DONE;");
    out.line("IF call.DONE THEN");
    out.depth += 1;
    match copied {
        [] => lifted.write(out, "call.w"),
        copied => {
            for (output, target) in copied {
                out.line(&format!("{target} := call.{};", output.name));
            }
        }
    }
    out.depth -= 1;
    out.line("END_IF;");
    out.depth -= 1;
}

/// `get`, the block that reads a future and lets go of its reading end:
/// `read`, the future's read, then `drop` in the call that sees `DONE`,
/// which sets `end` to 0, so that a later call reads nothing twice. Its
/// outputs are the read's, `outputs` past its `DONE`.
fn future_get(out: &mut Out, get: &str, read: &str, drop: &str, end: &str, outputs: &[Var]) {
    // The read keeps clear of the outputs' names and of the end's.
    let mut call = "call".to_string();
    while call.eq_ignore_ascii_case(end)
        || outputs
            .iter()
            .any(|var| var.name.eq_ignore_ascii_case(&call))
    {
        call.push('_');
    }
    let var = |name: &str, ty: &str| Var {
        name: name.to_string(),
        ty: ty.to_string(),
        flat: 1,
        start: None,
    };
    out.prose(&format!(
        "Reads the value the future holds, which comes back after `DONE`, then lets go of the future's reading end: `{end}` is 0 from then on, and a call with `{end}` 0 does nothing."
    ));
    out.line(&format!("FUNCTION_BLOCK {get}"));
    out.vars("VAR_IN_OUT", &[var(end, "UDINT")]);
    out.vars("VAR_OUTPUT", &[vec![done()], outputs.to_vec()].concat());
    out.vars("VAR", &[var(&call, read)]);
    out.depth += 1;
    out.line("DONE := FALSE;");
    out.line(&format!("IF {end} <> 0 THEN"));
    out.depth += 1;
    out.line(&format!("{call}({end} := {end});"));
    out.line(&format!("IF {call}.DONE THEN"));
    out.depth += 1;
    for output in outputs {
        out.line(&format!("{0} := {call}.{0};", output.name));
    }
    out.line(&format!("{drop}({end} := {end});"));
    out.line(&format!("{end} := 0;"));
    out.line("DONE := TRUE;");
    out.depth -= 1;
    out.line("END_IF;");
    out.depth -= 1;
    out.line("END_IF;");
    out.depth -= 1;
    out.line("END_FUNCTION_BLOCK");
    out.line("");
}

/// What a writer or a reader calls: its name, and its inputs, one per
/// parameter of its function, in order; for the block of an `async func`,
/// the outputs it gives past its `DONE`, and for a FUNCTION, `None`; a
/// FUNCTION's outputs, where it gives its values as outputs.
struct Declared {
    pou: String,
    inputs: Vec<Var>,
    block: Option<Vec<Var>>,
    given: Vec<Var>,
}

/// What `ty` is when it is a stream or a future, under aliases too: which,
/// and what it carries.
fn carried(resolve: &Resolve, ty: Type) -> Option<(Carrier, Option<Type>)> {
    let Type::Id(id) = ty else { return None };
    match &resolve.types[id].kind {
        TypeDefKind::Type(inner) => carried(resolve, *inner),
        TypeDefKind::Stream(payload) => Some((Carrier::Stream, *payload)),
        TypeDefKind::Future(payload) => Some((Carrier::Future, *payload)),
        _ => None,
    }
}

/// What the function a writer calls answers with.
enum Answer {
    /// Nothing.
    Nothing,
    /// A future no read gives back, let go of unread with this drop.
    Unread(String),
    /// A future, read by this `_GET`, whose outputs past `DONE` are these.
    Future(String, Vec<Var>),
    /// The result of an `async func`, the outputs of its block past `DONE`.
    Call(Vec<Var>),
}

/// The block that writes to `function`, `<NAME>_WRITER`, when the function
/// takes one stream of bytes from the program, and no other stream or
/// future: it makes the stream and writes it with the stream's
/// `BYTES_STREAM_WRITER` (`managers`). A `func` that answers with a future
/// or nothing is called with the stream's reading end, and its future read
/// with its `_GET`, keeping its value, or let go of unread when no read
/// gives it back; the call of an `async func` is started with it and
/// followed to its end once, keeping its result.
fn function_writer(
    blocks: &Blocks<'_>,
    out: &mut Out,
    taken: &mut HashMap<String, String>,
    function: &Function,
    declared: &Declared,
    form: Form,
) {
    let Blocks {
        resolve, managers, ..
    } = *blocks;
    let mut streams = function
        .params
        .iter()
        .enumerate()
        .filter(|(_, param)| carried(resolve, param.ty).is_some());
    let (Some((at, param)), None) = (streams.next(), streams.next()) else {
        return;
    };
    if carried(resolve, param.ty) != Some((Carrier::Stream, Some(Type::U8))) {
        return;
    }
    let Some(writer) = managers
        .get(&(Carrier::Stream, type_key(resolve, Some(Type::U8))))
        .and_then(|managers| match form {
            Form::Text => managers.writer.clone(),
            Form::Array => managers.array_writer.clone(),
        })
    else {
        return;
    };
    let answer = match (&declared.block, function.result) {
        // An `async func` whose result carries no stream or future.
        (Some(outputs), _) if function.find_futures_and_streams(resolve).len() == 1 => {
            Answer::Call(outputs.clone())
        }
        (Some(_), _) => return,
        (None, None) => Answer::Nothing,
        (None, Some(ty)) => match carried(resolve, ty) {
            Some((Carrier::Future, payload)) => {
                match managers.get(&(Carrier::Future, type_key(resolve, payload))) {
                    Some(Managers {
                        get: Some((get, outputs)),
                        ..
                    }) => Answer::Future(get.clone(), outputs.clone()),
                    Some(managers) => Answer::Unread(managers.drop_readable.clone()),
                    None => return,
                }
            }
            _ => return,
        },
    };
    let (suffix, what) = match form {
        Form::Text => ("WRITER", "writer"),
        Form::Array => ("ARRAY_WRITER", "array writer"),
    };
    let block = spare(taken, format!("{}_{suffix}", declared.pou));
    taken.insert(
        block.to_ascii_uppercase(),
        format!("the {what} of `{}`", function.name),
    );

    // The function by its namespace's path, which no name of the block's
    // own can hide: `OPEN`, beside the block's `open`.
    let pou = &blocks.called(&declared.pou);
    let shown = shown_name(resolve, function);
    let mut held = Held::new(
        match form {
            Form::Text => &[
                "REQ", "IN", "CLOSE", "BUSY", "DONE", "ERROR", "ERRORID", "ENDED", "open",
                "future", "stream", "answer", "call",
            ],
            Form::Array => &[
                "REQ", "IN", "COUNT", "CLOSE", "BUSY", "DONE", "ERROR", "ERRORID", "ENDED", "open",
                "future", "stream", "answer", "call",
            ],
        },
        "writer",
    );
    let var = |name: &str, ty: &str| Var {
        name: name.to_string(),
        ty: ty.to_string(),
        flat: 1,
        start: None,
    };
    let inputs = passed_inputs(resolve, declared, function, Some(at), &shown, &mut held);
    // What the block keeps of the answer, where it reads each value, and
    // the value's type in WIT.
    let (kept, from, answered): (&[Var], &str, Option<Type>) = match &answer {
        Answer::Future(_, outputs) => (
            outputs,
            "answer",
            function
                .result
                .and_then(|ty| carried(resolve, ty))
                .and_then(|(_, payload)| payload),
        ),
        Answer::Call(outputs) => (outputs, "call", function.result),
        Answer::Nothing | Answer::Unread(_) => (&[], "", None),
    };
    let Kept {
        values,
        flag,
        result,
    } = kept_values(resolve, kept, answered, &shown, &mut held);
    let documented = |var: Var, text: String| held.documented(var, text, &shown);
    let starts = match answer {
        Answer::Call(_) => "starts",
        _ => "calls",
    };
    let (moved, written) = match form {
        Form::Text => ("text", "`IN`"),
        Form::Array => ("bytes from an array", "the first `COUNT` bytes of `IN`"),
    };
    out.prose(&format!(
        "Writes {moved} to `{shown}` through a stream of bytes it opens at the first request."
    ));
    out.line(&format!("FUNCTION_BLOCK {block}"));
    let text = format!("STRING[{STRING_LIMIT}]");
    let req = documented(
        var("REQ", "BOOL"),
        format!(
            "A rising edge writes {written}; the first opens the stream and {starts} `{shown}` with it. An edge while `BUSY` is ignored."
        ),
    );
    let close = documented(var("CLOSE", "BOOL"), WRITER_CLOSE.to_string());
    let mut inputs_documented = match form {
        Form::Text => vec![
            req,
            documented(var("IN", &text), WRITER_IN.to_string()),
            close,
        ],
        Form::Array => vec![
            req,
            documented(var("COUNT", "UDINT"), WRITER_COUNT.to_string()),
            close,
        ],
    };
    inputs_documented.extend(
        inputs
            .iter()
            .map(|(var, _, doc)| documented(var.clone(), doc.clone())),
    );
    out.documented_vars("VAR_INPUT", &inputs_documented);
    if form == Form::Array {
        out.documented_vars(
            "VAR_IN_OUT",
            &[documented(
                var("IN", "ARRAY[*] OF BYTE"),
                WRITER_ARRAY_IN.to_string(),
            )],
        );
    }
    let mut outputs = vec![
        documented(
            var("BUSY", "BOOL"),
            match form {
                Form::Text => WRITER_BUSY,
                Form::Array => WRITER_ARRAY_BUSY,
            }
            .to_string(),
        ),
        documented(
            var("DONE", "BOOL"),
            match form {
                Form::Text => WRITER_DONE,
                Form::Array => WRITER_ARRAY_DONE,
            }
            .to_string(),
        ),
        documented(
            var("ERROR", "BOOL"),
            error_doc(
                resolve,
                Some("TRUE once a write finds the reader gone, a request comes after `CLOSE`"),
                result,
                &shown,
            )
            .unwrap_or_else(|| WRITER_ERROR.to_string()),
        ),
    ];
    // `ERRORID` beside `ERROR`, the answer's other values after `ENDED`.
    let (errorid, values_after): (Vec<_>, Vec<_>) =
        values.iter().partition(|(var, ..)| var.name == "ERRORID");
    outputs.extend(
        errorid
            .into_iter()
            .map(|(var, _, doc)| documented(var.clone(), doc.clone())),
    );
    let whole = wit_text(resolve, answered);
    match answer {
        Answer::Future(..) => outputs.push(documented(
            var("ENDED", "BOOL"),
            match answered {
                Some(_) => format!("TRUE once the `future<{whole}>` from `{shown}` has its value."),
                None => format!("TRUE once the `future` from `{shown}` has come."),
            },
        )),
        Answer::Call(_) => outputs.push(documented(
            var("ENDED", "BOOL"),
            format!("TRUE once `{shown}` has returned."),
        )),
        Answer::Nothing | Answer::Unread(_) => {}
    }
    outputs.extend(
        values_after
            .into_iter()
            .map(|(var, _, doc)| documented(var.clone(), doc.clone())),
    );
    out.documented_vars("VAR_OUTPUT", &outputs);
    let mut locals = vec![var("open", "BOOL"), var("stream", &writer)];
    match &answer {
        Answer::Future(get, _) => {
            locals.push(var("future", "UDINT"));
            locals.push(var("answer", get));
        }
        Answer::Call(_) => locals.push(var("call", pou)),
        Answer::Nothing | Answer::Unread(_) => {}
    }
    out.vars("VAR", &locals);
    out.depth += 1;
    let arguments = std::iter::once(format!("{} := stream.readable", declared.inputs[at].name))
        .chain(
            inputs
                .iter()
                .map(|(var, field, _)| format!("{field} := {}", var.name)),
        )
        .collect::<Vec<_>>()
        .join(", ");
    let keep = |out: &mut Out| {
        for (var, field, _) in &values {
            out.line(&format!("{} := {from}.{field};", var.name));
        }
        out.line("ENDED := TRUE;");
    };
    match answer {
        Answer::Call(_) => out.line("// The stream, made at the first request."),
        _ => out.line("// The stream, made at the first request and handed to the function."),
    }
    out.line("IF REQ AND NOT open THEN");
    out.depth += 1;
    // Every input passed, none requested yet: the call only makes the
    // stream.
    out.line(match form {
        Form::Text => "stream(REQ := FALSE, IN := IN, CLOSE := FALSE);",
        Form::Array => "stream(REQ := FALSE, COUNT := COUNT, CLOSE := FALSE, IN := IN);",
    });
    match &answer {
        Answer::Nothing => out.line(&format!("{pou}({arguments});")),
        Answer::Unread(drop) => out.line(&format!("{drop}(future := {pou}({arguments}));")),
        Answer::Future(..) => out.line(&format!("future := {pou}({arguments});")),
        Answer::Call(_) => {}
    }
    out.line("open := TRUE;");
    out.depth -= 1;
    out.line("END_IF;");
    out.line("IF open THEN");
    out.depth += 1;
    if let Answer::Call(_) = answer {
        // A call of the block once it has returned would start another.
        out.line("// The call, started with the stream's reading end at the first");
        out.line("// request and followed to its end, once.");
        out.line("IF NOT ENDED THEN");
        out.depth += 1;
        out.line(&format!("call({arguments});"));
        out.line("IF call.DONE THEN");
        out.depth += 1;
        keep(out);
        out.depth -= 1;
        out.line("END_IF;");
        out.depth -= 1;
        out.line("END_IF;");
    }
    out.line(match form {
        Form::Text => "stream(REQ := REQ, IN := IN, CLOSE := CLOSE);",
        Form::Array => "stream(REQ := REQ, COUNT := COUNT, CLOSE := CLOSE, IN := IN);",
    });
    out.line("BUSY := stream.BUSY;");
    out.line("DONE := stream.DONE;");
    if let Answer::Future(..) = answer {
        out.line("// The answer, read once it comes, and kept.");
        out.line("answer(future := future);");
        out.line("IF answer.DONE THEN");
        out.depth += 1;
        keep(out);
        out.depth -= 1;
        out.line("END_IF;");
    }
    match flag {
        Some(flag) => out.line(&format!("ERROR := stream.ERROR OR {from}.{};", flag.name)),
        None => out.line("ERROR := stream.ERROR;"),
    }
    out.depth -= 1;
    out.line("END_IF;");
    out.depth -= 1;
    out.line("END_FUNCTION_BLOCK");
    out.line("");
}

/// What the blocks written for an interface's functions read: its WIT,
/// the blocks of each set of built-ins, and the path of its namespace.
struct Blocks<'a> {
    resolve: &'a Resolve,
    managers: &'a HashMap<(Carrier, String), Managers>,
    namespace: &'a str,
}

impl Blocks<'_> {
    /// `pou`, a POU of the interface, named by its namespace's path.
    fn called(&self, pou: &str) -> String {
        format!("{}.{pou}", self.namespace)
    }
}

/// The function as WIT names it: a resource's `send` is `channel.send`.
fn shown_name(resolve: &Resolve, function: &Function) -> String {
    match function.kind {
        FunctionKind::Method(id)
        | FunctionKind::AsyncMethod(id)
        | FunctionKind::Static(id)
        | FunctionKind::AsyncStatic(id) => format!(
            "{}.{}",
            wit_text(resolve, Some(Type::Id(id))),
            function.item_name()
        ),
        _ => function.item_name().to_string(),
    }
}

/// The names a block written for a function declares, its own first: a
/// WIT name that would be one of them, or one taken before it, takes a
/// `_`, and the documentation of each of the two names the other. A name
/// the function's own declaration took apart counts too: `done_` of an
/// `async func`'s block, beside its `DONE`.
struct Held {
    own: &'static [&'static str],
    /// What the block is, `writer` or `reader`.
    whose: &'static str,
    held: Vec<String>,
    /// Each name taken apart: the name it met, its own, and where it comes
    /// from.
    met: Vec<(String, String, String)>,
}

impl Held {
    fn new(own: &'static [&'static str], whose: &'static str) -> Self {
        Held {
            own,
            whose,
            held: own.iter().map(|name| name.to_string()).collect(),
            met: Vec::new(),
        }
    }

    /// `name` as the block declares it, apart from the names held; `from`
    /// says where it comes from.
    fn apart(&mut self, name: &str, from: String) -> String {
        let first = self
            .held
            .iter()
            .find(|other| other.eq_ignore_ascii_case(name.trim_end_matches('_')))
            .cloned();
        let mut new = name.to_string();
        while self
            .held
            .iter()
            .any(|other| other.eq_ignore_ascii_case(&new))
        {
            new.push('_');
        }
        self.held.push(new.clone());
        if let Some(first) = first {
            self.met.push((first, new.clone(), from));
        }
        new
    }

    /// `var` with its documentation, `text`, shown on hover, and for a name
    /// taken apart, the other of the two.
    fn documented(&self, var: Var, text: String, shown: &str) -> (Var, String) {
        let mut text = text;
        for (first, new, from) in &self.met {
            if *first == var.name {
                text.push_str(&format!(" `{new}` is {from}."));
            }
            if *new == var.name {
                let other = match self.own.contains(&first.as_str()) {
                    true => format!("the {}'s own", self.whose),
                    false => self
                        .met
                        .iter()
                        .find(|(_, new, _)| new == first)
                        .map(|(.., from)| from.clone())
                        .unwrap_or_else(|| format!("a parameter of `{shown}`")),
                };
                text.push_str(&format!(" Not `{first}`, {other}."));
            }
        }
        (var, text)
    }
}

/// The function's inputs, but the one at `skip`, which a block passes it
/// when it opens its stream: the block's input, the function's name of
/// it, and what it is in WIT.
fn passed_inputs(
    resolve: &Resolve,
    declared: &Declared,
    function: &Function,
    skip: Option<usize>,
    shown: &str,
    held: &mut Held,
) -> Vec<(Var, String, String)> {
    declared
        .inputs
        .iter()
        .zip(&function.params)
        .enumerate()
        .filter(|(index, _)| Some(*index) != skip)
        .map(|(_, (input, param))| {
            let name = held.apart(&input.name, format!("a parameter of `{shown}`"));
            let doc = format!(
                "`{}: {}` of `{shown}`, passed when the stream opens.",
                param.name,
                wit_text(resolve, Some(param.ty))
            );
            (
                Var {
                    name,
                    ..input.clone()
                },
                input.name.clone(),
                doc,
            )
        })
        .collect()
}

/// What a block keeps of a function's answer: each value, with the name
/// the block gives it, its name where the answer is, and what it is in
/// WIT; a result's flag, which turns the block's `ERROR` TRUE; and the
/// result, with its error, the block's `ERRORID`.
struct Kept {
    values: Vec<(Var, String, String)>,
    flag: Option<Var>,
    result: Option<(Type, Option<Type>)>,
}

/// What a block keeps of `kept`, the outputs a function's answer comes
/// back in, a value of `answered`.
fn kept_values(
    resolve: &Resolve,
    kept: &[Var],
    answered: Option<Type>,
    shown: &str,
    held: &mut Held,
) -> Kept {
    let result = answered.and_then(|ty| match defined(resolve, ty) {
        Some(TypeDefKind::Result(result)) => Some((ty, result.err)),
        _ => None,
    });
    let is = |output: &Var, name: &str| {
        result.is_some() && output.name.trim_end_matches('_').eq_ignore_ascii_case(name)
    };
    let flag = kept.iter().find(|output| is(output, "failed")).cloned();
    let values = kept
        .iter()
        .filter(|output| !is(output, "failed"))
        .map(|output| {
            let doc = match answered {
                Some(ty) => part_of(resolve, ty, &output.name, shown),
                None => String::new(),
            };
            let name = match is(output, "error") {
                true => "ERRORID".to_string(),
                false => held.apart(&output.name, format!("from `{shown}`")),
            };
            (
                Var {
                    name,
                    ..output.clone()
                },
                output.name.clone(),
                doc,
            )
        })
        .collect();
    Kept {
        values,
        flag,
        result,
    }
}

/// What a block's `ERROR` says when the answer is a result: its own
/// `causes`, if any, then the result's error.
fn error_doc(
    resolve: &Resolve,
    causes: Option<&str>,
    result: Option<(Type, Option<Type>)>,
    shown: &str,
) -> Option<String> {
    let (ty, err) = result?;
    let causes = match causes {
        Some(causes) => format!("{causes}, or"),
        None => "TRUE once".to_string(),
    };
    Some(format!(
        "{causes} the `{}` from `{shown}` is an error{}.",
        wit_text(resolve, Some(ty)),
        match err {
            Some(err) => format!(", whose `{}` is in `ERRORID`", wit_text(resolve, Some(err))),
            None => String::new(),
        }
    ))
}

/// The block that reads what `function` gives, `<NAME>_READER`, when the
/// function is a `func` that takes no stream or future and gives one
/// stream of bytes, alone or in a tuple beside one future, which `managers`
/// read: at the first request it calls the function, then reads at each
/// rising edge of `REQ` with the stream's `_READ`, into a STRING, and lets
/// go of the reading end once the writer left or on `CLOSE`. The future is
/// read with its `_GET`, keeping its value, a result's error in `ERROR`
/// and `ERRORID`, or let go of unread when no read gives it back.
fn function_reader(
    blocks: &Blocks<'_>,
    out: &mut Out,
    taken: &mut HashMap<String, String>,
    function: &Function,
    declared: &Declared,
    form: Form,
) {
    let Blocks {
        resolve, managers, ..
    } = *blocks;
    if declared.block.is_some() {
        return;
    }
    let mut params = function.clone();
    params.result = None;
    if !params.find_futures_and_streams(resolve).is_empty() {
        return;
    }
    let Some(result) = function.result else {
        return;
    };
    let bytes = Some((Carrier::Stream, Some(Type::U8)));
    // Where the stream and the future come back: the FUNCTION's own value,
    // or two of its outputs, as the tuple orders them.
    let (call_gives, future): (Option<(String, String)>, Option<Option<Type>>) =
        if carried(resolve, result) == bytes {
            (None, None)
        } else {
            let Some(TypeDefKind::Tuple(tuple)) = defined(resolve, result) else {
                return;
            };
            let [first, second] = tuple.types[..] else {
                return;
            };
            let (at, payload) = match (carried(resolve, first), carried(resolve, second)) {
                (stream, Some((Carrier::Future, payload))) if stream == bytes => ((0, 1), payload),
                (Some((Carrier::Future, payload)), stream) if stream == bytes => ((1, 0), payload),
                _ => return,
            };
            let (Some(stream), Some(future)) = (declared.given.get(at.0), declared.given.get(at.1))
            else {
                return;
            };
            (
                Some((stream.name.clone(), future.name.clone())),
                Some(payload),
            )
        };
    let Some((read, drop)) = managers
        .get(&(Carrier::Stream, type_key(resolve, Some(Type::U8))))
        .and_then(|managers| {
            let read = match form {
                Form::Text => managers.read.clone(),
                Form::Array => managers.read_array.clone(),
            };
            Some((read?, managers.drop_readable.clone()))
        })
    else {
        return;
    };
    let answer = match future {
        None => Answer::Nothing,
        Some(payload) => match managers.get(&(Carrier::Future, type_key(resolve, payload))) {
            Some(Managers {
                get: Some((get, outputs)),
                ..
            }) => Answer::Future(get.clone(), outputs.clone()),
            Some(managers) => Answer::Unread(managers.drop_readable.clone()),
            None => return,
        },
    };
    let (suffix, what) = match form {
        Form::Text => ("READER", "reader"),
        Form::Array => ("ARRAY_READER", "array reader"),
    };
    let block = spare(taken, format!("{}_{suffix}", declared.pou));
    taken.insert(
        block.to_ascii_uppercase(),
        format!("the {what} of `{}`", function.name),
    );

    // The function by its namespace's path, which no name of the block's
    // own can hide: `OPEN`, beside the block's `open`.
    let pou = &blocks.called(&declared.pou);
    let shown = shown_name(resolve, function);
    let mut held = Held::new(
        &[
            "REQ",
            "CLOSE",
            "BUSY",
            "DONE",
            "OUT",
            "COUNT",
            "ERROR",
            "ERRORID",
            "ENDED",
            "open",
            "requested",
            "closing",
            "shut",
            "came",
            "stream",
            "future",
            "read",
            "answer",
        ],
        "reader",
    );
    let var = |name: &str, ty: &str| Var {
        name: name.to_string(),
        ty: ty.to_string(),
        flat: 1,
        start: None,
    };
    let inputs = passed_inputs(resolve, declared, function, None, &shown, &mut held);
    let (kept, answered): (&[Var], Option<Type>) = match &answer {
        Answer::Future(_, outputs) => (outputs, future.flatten()),
        _ => (&[], None),
    };
    let Kept {
        values,
        flag,
        result,
    } = kept_values(resolve, kept, answered, &shown, &mut held);
    let documented = |var: Var, text: String| held.documented(var, text, &shown);

    let (into, at_most, nothing) = match form {
        Form::Text => ("", STRING_LIMIT, "`OUT` empty"),
        Form::Array => (" into an array", BYTES_AT_ONCE, "`COUNT` 0"),
    };
    out.prose(&format!(
        "Reads what `{shown}` gives through a stream of bytes{into}, a request at a time."
    ));
    out.line(&format!("FUNCTION_BLOCK {block}"));
    let mut inputs_documented = vec![
        documented(
            var("REQ", "BOOL"),
            format!(
                "A rising edge reads what comes next, {at_most} bytes at most; the first calls `{shown}`. An edge while `BUSY` is ignored, and one after the stream ended is `DONE` at once, {nothing}."
            ),
        ),
        documented(
            var("CLOSE", "BOOL"),
            "A rising edge lets go of the stream, once no read is under way.".to_string(),
        ),
    ];
    inputs_documented.extend(
        inputs
            .iter()
            .map(|(var, _, doc)| documented(var.clone(), doc.clone())),
    );
    out.documented_vars("VAR_INPUT", &inputs_documented);
    let mut outputs: Vec<(Var, String)> = vec![
        documented(
            var("BUSY", "BOOL"),
            "TRUE while a read waits for bytes.".to_string(),
        ),
        documented(
            var("DONE", "BOOL"),
            "TRUE in the call where a read has given what came, in `OUT`.".to_string(),
        ),
    ];
    match form {
        Form::Text => outputs.push(documented(
            var("OUT", &format!("STRING[{STRING_LIMIT}]")),
            "What the last read gave: empty once the stream has ended, nothing more coming."
                .to_string(),
        )),
        Form::Array => {
            outputs.push(documented(
                var("OUT", &format!("ARRAY[0..{}] OF BYTE", BYTES_AT_ONCE - 1)),
                "The bytes the last read gave, its first `COUNT`.".to_string(),
            ));
            outputs.push(documented(
                var("COUNT", "UDINT"),
                "How many bytes the last read gave: 0 once the stream has ended, nothing more coming."
                    .to_string(),
            ));
        }
    }
    // A result's error, the one the reader reports.
    if let Some(doc) = error_doc(resolve, None, result, &shown) {
        outputs.push(documented(var("ERROR", "BOOL"), doc));
    }
    // `ERRORID` beside `ERROR`, the answer's other values after `ENDED`.
    let (errorid, values_after): (Vec<_>, Vec<_>) =
        values.iter().partition(|(var, ..)| var.name == "ERRORID");
    outputs.extend(
        errorid
            .into_iter()
            .map(|(var, _, doc)| documented(var.clone(), doc.clone())),
    );
    outputs.push(documented(
        var("ENDED", "BOOL"),
        match (&answer, answered) {
            (Answer::Future(..), Some(ty)) => format!(
                "TRUE once the stream has ended and the `future<{}>` from `{shown}` has its value.",
                wit_text(resolve, Some(ty))
            ),
            (Answer::Future(..), None) => {
                format!("TRUE once the stream has ended and the `future` from `{shown}` has come.")
            }
            _ => "TRUE once the stream has ended: the writer let go, or `CLOSE`.".to_string(),
        },
    ));
    outputs.extend(
        values_after
            .into_iter()
            .map(|(var, _, doc)| documented(var.clone(), doc.clone())),
    );
    out.documented_vars("VAR_OUTPUT", &outputs);
    let mut locals = vec![
        var("open", "BOOL"),
        var("requested", "BOOL"),
        var("closing", "BOOL"),
        var("shut", "BOOL"),
        var("stream", "UDINT"),
    ];
    if call_gives.is_some() {
        locals.push(var("future", "UDINT"));
    }
    locals.push(var("read", &read));
    if let Answer::Future(get, _) = &answer {
        locals.push(var("answer", get));
        locals.push(var("came", "BOOL"));
    }
    out.vars("VAR", &locals);
    out.depth += 1;
    let mut arguments: Vec<String> = inputs
        .iter()
        .map(|(var, field, _)| format!("{field} := {}", var.name))
        .collect();
    out.line("DONE := FALSE;");
    out.line("// The stream, given by the function at the first request.");
    out.line("IF REQ AND NOT open THEN");
    out.depth += 1;
    match &call_gives {
        None => out.line(&format!("stream := {pou}({});", arguments.join(", "))),
        Some((stream, future)) => {
            arguments.push(format!("{stream} => stream"));
            arguments.push(format!("{future} => future"));
            out.line(&format!("{pou}({});", arguments.join(", ")));
        }
    }
    if let Answer::Unread(drop) = &answer {
        out.line("// No read gives the future's value back: it goes unread.");
        out.line(&format!("{drop}(future := future);"));
    }
    out.line("open := TRUE;");
    out.depth -= 1;
    out.line("END_IF;");
    out.line("IF open THEN");
    out.depth += 1;
    let lines = [
        (0, "// An edge while BUSY is ignored."),
        (0, "IF REQ AND NOT requested AND NOT BUSY THEN"),
        (1, "IF stream <> 0 THEN"),
        (2, "BUSY := TRUE;"),
        (1, "ELSE"),
        (2, "// The stream has ended: nothing more comes."),
        (
            2,
            match form {
                Form::Text => "OUT := '';",
                Form::Array => "COUNT := 0;",
            },
        ),
        (2, "DONE := TRUE;"),
        (1, "END_IF;"),
        (0, "END_IF;"),
        (0, "requested := REQ;"),
        (0, "IF BUSY THEN"),
        (1, "// Starts the read, or checks the one running."),
        (1, "read(stream := stream);"),
        (1, "IF read.DONE THEN"),
        (2, "OUT := read.data;"),
        (
            2,
            match form {
                Form::Text => "",
                Form::Array => "COUNT := read.read;",
            },
        ),
        (2, "BUSY := FALSE;"),
        (2, "DONE := TRUE;"),
        (2, "IF read.closed THEN"),
        (3, "// The writer left: the reading end goes too."),
    ];
    // The text form has no count to set.
    for (depth, line) in lines.into_iter().filter(|(_, line)| !line.is_empty()) {
        out.depth += depth;
        out.line(line);
        out.depth -= depth;
    }
    out.depth += 3;
    out.line(&format!("{drop}(stream := stream);"));
    out.line("stream := 0;");
    out.depth -= 3;
    let lines = [
        (2, "END_IF;"),
        (1, "END_IF;"),
        (0, "END_IF;"),
        (
            0,
            "// The reading end goes on CLOSE, once no read is under way.",
        ),
        (0, "IF CLOSE AND NOT closing THEN"),
        (1, "shut := TRUE;"),
        (0, "END_IF;"),
        (0, "closing := CLOSE;"),
        (0, "IF shut AND NOT BUSY THEN"),
        (1, "IF stream <> 0 THEN"),
    ];
    for (depth, line) in lines {
        out.depth += depth;
        out.line(line);
        out.depth -= depth;
    }
    out.depth += 2;
    out.line(&format!("{drop}(stream := stream);"));
    out.line("stream := 0;");
    out.depth -= 2;
    for (depth, line) in [(1, "END_IF;"), (1, "shut := FALSE;"), (0, "END_IF;")] {
        out.depth += depth;
        out.line(line);
        out.depth -= depth;
    }
    if let Answer::Future(..) = answer {
        out.line("// The answer, read once it comes, and kept.");
        out.line("answer(future := future);");
        out.line("IF answer.DONE THEN");
        out.depth += 1;
        for (var, field, _) in &values {
            out.line(&format!("{} := answer.{field};", var.name));
        }
        out.line("came := TRUE;");
        out.depth -= 1;
        out.line("END_IF;");
        out.line("ENDED := stream = 0 AND came;");
    } else {
        out.line("ENDED := stream = 0;");
    }
    if let Some(flag) = flag {
        out.line(&format!("ERROR := answer.{};", flag.name));
    }
    out.depth -= 1;
    out.line("END_IF;");
    out.depth -= 1;
    out.line("END_FUNCTION_BLOCK");
    out.line("");
}

/// What the inputs and outputs every writer of a stream of bytes declares
/// are, shown on hover.
const WRITER_IN: &str = "The text to write.";
const WRITER_CLOSE: &str = "A rising edge ends the stream, once no write is under way.";
const WRITER_BUSY: &str = "TRUE while the reader has not taken all of `IN`.";
const WRITER_DONE: &str =
    "TRUE in the call where the reader has taken all of `IN`, at once for an empty text.";
const WRITER_ERROR: &str = "TRUE once a write finds the reader gone, or a request comes after `CLOSE`: the stream does not open again.";

/// What `ty` is defined as, past its aliases; `None` for a primitive type.
fn defined(resolve: &Resolve, ty: Type) -> Option<&TypeDefKind> {
    let mut at = ty;
    while let Type::Id(id) = at {
        match &resolve.types[id].kind {
            TypeDefKind::Type(inner) => at = *inner,
            other => return Some(other),
        }
    }
    None
}

/// What `output`, one of the outputs a value of `ty` comes back in, holds
/// of it, in WIT's terms: a result's flag and its sides, an option's flag
/// and value, a tuple's items, or the value itself (`shapes::result`); the
/// value comes from the function `shown`.
fn part_of(resolve: &Resolve, ty: Type, output: &str, shown: &str) -> String {
    let whole = wit_text(resolve, Some(ty));
    let is = |name: &str| output.trim_end_matches('_').eq_ignore_ascii_case(name);
    let kind = defined(resolve, ty);
    let from = format!("the `{whole}` from `{shown}`");
    match kind {
        Some(TypeDefKind::Result(_)) if is("failed") => format!("TRUE when {from} is an error."),
        Some(TypeDefKind::Result(result)) if is("error") => format!(
            "The `{}` of {from}, when it is an error, which turns `ERROR` TRUE.",
            wit_text(resolve, result.err)
        ),
        Some(TypeDefKind::Result(result)) if is("value") => format!(
            "The `{}` of {from}, when it is no error.",
            wit_text(resolve, result.ok)
        ),
        Some(TypeDefKind::Option(_)) if is("is_some") => {
            format!("TRUE when {from} holds a value.")
        }
        Some(TypeDefKind::Option(inner)) if is("value") => format!(
            "The `{}` that {from} holds.",
            wit_text(resolve, Some(*inner))
        ),
        Some(TypeDefKind::Tuple(tuple)) => {
            let item = output
                .trim_end_matches('_')
                .strip_prefix("item")
                .and_then(|at| at.parse::<usize>().ok())
                .and_then(|at| tuple.types.get(at).map(|ty| (at, ty)));
            match item {
                Some((at, ty)) => {
                    format!("Item {at} of {from}, a `{}`.", wit_text(resolve, Some(*ty)))
                }
                None => format!("Part of {from}."),
            }
        }
        Some(TypeDefKind::Result(_) | TypeDefKind::Option(_)) => format!("Part of {from}."),
        _ if is("value") => format!("The `{whole}` from `{shown}`."),
        _ => format!("Part of {from}."),
    }
}

/// The names a stream's writer uses: its own, those of its two helpers, a
/// text's length in bytes and what is left of it, and those of the
/// built-ins it calls.
struct Writer {
    writer: String,
    len: String,
    after: String,
    new: String,
    write: String,
    drop: String,
}

/// The block that writes a stream of bytes it makes itself, `names.writer`,
/// and its two helpers, private to the interface's namespace: the work
/// `Std.Console`'s blocks did by hand, a write handing the reader what it
/// has not taken until it has all of it.
fn stream_writer(out: &mut Out, names: &Writer) {
    let Writer {
        writer,
        len,
        after,
        new,
        write,
        drop,
    } = names;
    let var = |name: &str, ty: &str| Var {
        name: name.to_string(),
        ty: ty.to_string(),
        flat: 1,
        start: None,
    };
    let text = format!("STRING[{STRING_LIMIT}]");

    out.prose(&format!("The bytes `IN` holds, which `{writer}` counts."));
    out.line(&format!("FUNCTION PRIVATE {len} : UDINT"));
    out.vars("VAR_INPUT", &[var("IN", &text)]);
    out.depth += 1;
    out.line(&format!(
        "{{wasm 'str.byte_len' (params IN) (result {len})}}"
    ));
    out.depth -= 1;
    out.line("END_FUNCTION");
    out.line("");

    out.prose(&format!(
        "`IN` without its first `L` bytes: what `{writer}` has left to write."
    ));
    out.line(&format!("FUNCTION PRIVATE {after} : {text}"));
    out.vars("VAR_INPUT", &[var("IN", &text), var("L", "UDINT")]);
    out.vars(
        "VAR",
        &[Var {
            start: Some("1"),
            ..var("P", "UDINT")
        }],
    );
    out.depth += 1;
    out.line(&format!(
        "{{wasm 'str.byte_delete' (params IN L P) (result {after})}}"
    ));
    out.depth -= 1;
    out.line("END_FUNCTION");
    out.line("");

    out.prose(
        "Writes text to a stream of bytes it makes at its first call, for the function `readable` is handed to.",
    );
    out.line(&format!("FUNCTION_BLOCK {writer}"));
    let documented = |name: &str, ty: &str, doc: &str| (var(name, ty), doc.to_string());
    out.documented_vars(
        "VAR_INPUT",
        &[
            documented(
                "REQ",
                "BOOL",
                "A rising edge writes `IN`. An edge while `BUSY` is ignored.",
            ),
            documented("IN", &text, WRITER_IN),
            documented("CLOSE", "BOOL", WRITER_CLOSE),
        ],
    );
    out.documented_vars(
        "VAR_OUTPUT",
        &[
            documented(
                "readable",
                "UDINT",
                "The stream's reading end, to hand to the function that reads it.",
            ),
            documented("BUSY", "BOOL", WRITER_BUSY),
            documented("DONE", "BOOL", WRITER_DONE),
            documented("ERROR", "BOOL", WRITER_ERROR),
        ],
    );
    out.vars(
        "VAR",
        &[
            var("writable", "UDINT"),
            var("requested", "BOOL"),
            var("closing", "BOOL"),
            var("shut", "BOOL"),
            var("closed", "BOOL"),
            var("rest", &text),
            var("write", write),
        ],
    );
    out.vars("VAR_TEMP", &[var("again", "BOOL")]);
    let body = [
        (0, "IF writable = 0 AND NOT closed THEN".to_string()),
        (
            1,
            format!("{new}(readable => readable, writable => writable);"),
        ),
        (0, "END_IF;".to_string()),
        (0, "DONE := FALSE;".to_string()),
        (0, "// An edge while BUSY is ignored.".to_string()),
        (0, "IF REQ AND NOT requested AND NOT BUSY THEN".to_string()),
        (1, "ERROR := closed;".to_string()),
        (1, "IF NOT closed THEN".to_string()),
        (
            2,
            "// A write of no bytes asks the reader whether it is ready,".to_string(),
        ),
        (
            2,
            "// which it may answer late: an empty text is done here.".to_string(),
        ),
        (2, format!("IF {len}(IN) = 0 THEN")),
        (3, "DONE := TRUE;".to_string()),
        (2, "ELSE".to_string()),
        (3, "rest := IN;".to_string()),
        (3, "BUSY := TRUE;".to_string()),
        (2, "END_IF;".to_string()),
        (1, "END_IF;".to_string()),
        (0, "END_IF;".to_string()),
        (0, "requested := REQ;".to_string()),
        (0, "IF BUSY THEN".to_string()),
        (
            1,
            "// Each pass after the first hands over a byte at least, so the".to_string(),
        ),
        (1, "// loop ends.".to_string()),
        (1, "REPEAT".to_string()),
        (2, "again := FALSE;".to_string()),
        (
            2,
            "// Starts the write of the rest, or checks the one running.".to_string(),
        ),
        (2, "write(stream := writable, data := rest);".to_string()),
        (2, "IF write.DONE THEN".to_string()),
        (3, "IF write.closed THEN".to_string()),
        (
            4,
            "// The reader left: the writing end goes too.".to_string(),
        ),
        (4, format!("{drop}(stream := writable);")),
        (4, "writable := 0;".to_string()),
        (4, "closed := TRUE;".to_string()),
        (4, "BUSY := FALSE;".to_string()),
        (4, "ERROR := TRUE;".to_string()),
        (3, format!("ELSIF write.written < {len}(rest) THEN")),
        (
            4,
            "// The reader took a part: the rest goes next, at once while".to_string(),
        ),
        (4, "// it takes some.".to_string()),
        (4, format!("rest := {after}(rest, write.written);")),
        (4, "again := write.written > 0;".to_string()),
        (3, "ELSE".to_string()),
        (4, "BUSY := FALSE;".to_string()),
        (4, "DONE := TRUE;".to_string()),
        (3, "END_IF;".to_string()),
        (2, "END_IF;".to_string()),
        (1, "UNTIL NOT again".to_string()),
        (1, "END_REPEAT;".to_string()),
        (0, "END_IF;".to_string()),
        (
            0,
            "// The writing end goes once no write is under way.".to_string(),
        ),
        (0, "IF CLOSE AND NOT closing THEN".to_string()),
        (1, "shut := TRUE;".to_string()),
        (0, "END_IF;".to_string()),
        (0, "closing := CLOSE;".to_string()),
        (0, "IF shut AND NOT BUSY THEN".to_string()),
        (1, "IF writable <> 0 THEN".to_string()),
        (2, format!("{drop}(stream := writable);")),
        (2, "writable := 0;".to_string()),
        (1, "END_IF;".to_string()),
        (1, "closed := TRUE;".to_string()),
        (1, "shut := FALSE;".to_string()),
        (0, "END_IF;".to_string()),
    ];
    let depth = out.depth + 1;
    for (indent, line) in body {
        out.depth = depth + indent;
        out.line(&line);
    }
    out.depth = depth - 1;
    out.line("END_FUNCTION_BLOCK");
    out.line("");
}

/// What the inputs and outputs of a writer of bytes from an array say
/// beside the text writer's.
const WRITER_COUNT: &str = "How many bytes of `IN` a request writes, from its first on.";
const WRITER_ARRAY_IN: &str = "The bytes to write. The host reads them where they are while `BUSY`: they stay as they are until `DONE`.";
const WRITER_ARRAY_BUSY: &str = "TRUE while the reader has not taken all `COUNT` bytes.";
const WRITER_ARRAY_DONE: &str =
    "TRUE in the call where the reader has taken all `COUNT` bytes, at once for none.";

/// What a writer or a reader moves: text, through a STRING, or bytes,
/// through an array of them and a count.
#[derive(Clone, Copy, PartialEq)]
enum Form {
    Text,
    Array,
}

/// The names a stream's array writer uses: its own, those of its two
/// helpers, an array's first subscript and its length, and those of the
/// built-ins it calls.
struct ArrayWriter {
    writer: String,
    first: String,
    len: String,
    new: String,
    write: String,
    drop: String,
}

/// The block that writes bytes from an array to a stream of bytes it makes
/// itself, `names.writer`, as the text writer does (`stream_writer`), and
/// its two helpers, private to the interface's namespace: the first
/// subscript of the array and how many bytes it holds. They read the
/// bounds with `{wasm}` intrinsics, as `Std.Arrays` does, so that the
/// declarations need no library but their own, as the text writer's
/// helpers. The host reads the bytes in place, from a reference to the
/// first not taken yet; a count past the array raises, as a subscript past
/// its bounds does.
fn stream_array_writer(out: &mut Out, names: &ArrayWriter) {
    let ArrayWriter {
        writer,
        first,
        len,
        new,
        write,
        drop,
    } = names;
    let var = |name: &str, ty: &str| Var {
        name: name.to_string(),
        ty: ty.to_string(),
        flat: 1,
        start: None,
    };
    let array = "ARRAY[*] OF BYTE";
    // The bounds as LINTs, which hold any bound and the length: a bound
    // that does not fit its variable raises. A COUNT compares with the
    // length with no conversion.
    let dimension = Var {
        start: Some("1"),
        ..var("DIM", "DINT")
    };
    out.prose(&format!(
        "The subscript of the first byte of `IN`, which `{writer}` writes from."
    ));
    out.line(&format!("FUNCTION PRIVATE {first} : LINT"));
    out.vars("VAR_INPUT", &[var("IN", array)]);
    out.vars("VAR", std::slice::from_ref(&dimension));
    out.depth += 1;
    out.line(&format!(
        "{{wasm 'array.lower_bound' (params IN DIM) (result {first})}}"
    ));
    out.depth -= 1;
    out.line("END_FUNCTION");
    out.line("");

    out.prose(&format!(
        "How many bytes `IN` holds, which `{writer}` writes at most."
    ));
    out.line(&format!("FUNCTION PRIVATE {len} : LINT"));
    out.vars("VAR_INPUT", &[var("IN", array)]);
    out.vars(
        "VAR",
        &[dimension, var("lower", "LINT"), var("upper", "LINT")],
    );
    out.depth += 1;
    out.line("{wasm 'array.lower_bound' (params IN DIM) (result lower)}");
    out.line("{wasm 'array.upper_bound' (params IN DIM) (result upper)}");
    out.line(&format!("{len} := upper - lower + 1;"));
    out.depth -= 1;
    out.line("END_FUNCTION");
    out.line("");

    out.prose(
        "Writes bytes from an array to a stream of bytes it makes at its first call, for the function `readable` is handed to.",
    );
    out.line(&format!("FUNCTION_BLOCK {writer}"));
    let documented = |name: &str, ty: &str, doc: &str| (var(name, ty), doc.to_string());
    out.documented_vars(
        "VAR_INPUT",
        &[
            documented(
                "REQ",
                "BOOL",
                "A rising edge writes the first `COUNT` bytes of `IN`. An edge while `BUSY` is ignored.",
            ),
            documented("COUNT", "UDINT", WRITER_COUNT),
            documented("CLOSE", "BOOL", WRITER_CLOSE),
        ],
    );
    out.documented_vars("VAR_IN_OUT", &[documented("IN", array, WRITER_ARRAY_IN)]);
    out.documented_vars(
        "VAR_OUTPUT",
        &[
            documented(
                "readable",
                "UDINT",
                "The stream's reading end, to hand to the function that reads it.",
            ),
            documented("BUSY", "BOOL", WRITER_ARRAY_BUSY),
            documented("DONE", "BOOL", WRITER_ARRAY_DONE),
            documented("ERROR", "BOOL", WRITER_ERROR),
        ],
    );
    out.vars(
        "VAR",
        &[
            var("writable", "UDINT"),
            var("requested", "BOOL"),
            var("closing", "BOOL"),
            var("shut", "BOOL"),
            var("closed", "BOOL"),
            var("total", "UDINT"),
            var("sent", "UDINT"),
            var("base", "LINT"),
            var("write", write),
        ],
    );
    out.vars("VAR_TEMP", &[var("again", "BOOL"), var("offset", "LINT")]);
    let body = [
        (0, "IF writable = 0 AND NOT closed THEN".to_string()),
        (
            1,
            format!("{new}(readable => readable, writable => writable);"),
        ),
        (0, "END_IF;".to_string()),
        (0, "DONE := FALSE;".to_string()),
        (0, "// An edge while BUSY is ignored.".to_string()),
        (0, "IF REQ AND NOT requested AND NOT BUSY THEN".to_string()),
        (1, "ERROR := closed;".to_string()),
        (1, "IF NOT closed THEN".to_string()),
        (
            2,
            "// A write of no bytes asks the reader whether it is ready,".to_string(),
        ),
        (
            2,
            "// which it may answer late: none is done here.".to_string(),
        ),
        (2, "IF COUNT = 0 THEN".to_string()),
        (3, "DONE := TRUE;".to_string()),
        (2, format!("ELSIF COUNT > {len}(IN) THEN")),
        (3, "__RAISE('COUNT is past the end of IN');".to_string()),
        (2, "ELSE".to_string()),
        (3, "total := COUNT;".to_string()),
        (3, "sent := 0;".to_string()),
        (3, format!("base := {first}(IN);")),
        (3, "BUSY := TRUE;".to_string()),
        (2, "END_IF;".to_string()),
        (1, "END_IF;".to_string()),
        (0, "END_IF;".to_string()),
        (0, "requested := REQ;".to_string()),
        (0, "IF BUSY THEN".to_string()),
        (
            1,
            "// Each pass after the first hands over a byte at least, so the".to_string(),
        ),
        (1, "// loop ends.".to_string()),
        (1, "REPEAT".to_string()),
        (2, "again := FALSE;".to_string()),
        (2, "offset := sent;".to_string()),
        (
            2,
            "// Starts the write of the bytes not taken yet, from where they".to_string(),
        ),
        (2, "// are, or checks the one running.".to_string()),
        (
            2,
            "write(stream := writable, data := REF(IN[base + offset]), count := total - sent);"
                .to_string(),
        ),
        (2, "IF write.DONE THEN".to_string()),
        (3, "IF write.closed THEN".to_string()),
        (
            4,
            "// The reader left: the writing end goes too.".to_string(),
        ),
        (4, format!("{drop}(stream := writable);")),
        (4, "writable := 0;".to_string()),
        (4, "closed := TRUE;".to_string()),
        (4, "BUSY := FALSE;".to_string()),
        (4, "ERROR := TRUE;".to_string()),
        (3, "ELSIF write.written < total - sent THEN".to_string()),
        (
            4,
            "// The reader took a part: the rest goes next, at once while".to_string(),
        ),
        (4, "// it takes some.".to_string()),
        (4, "sent := sent + write.written;".to_string()),
        (4, "again := write.written > 0;".to_string()),
        (3, "ELSE".to_string()),
        (4, "BUSY := FALSE;".to_string()),
        (4, "DONE := TRUE;".to_string()),
        (3, "END_IF;".to_string()),
        (2, "END_IF;".to_string()),
        (1, "UNTIL NOT again".to_string()),
        (1, "END_REPEAT;".to_string()),
        (0, "END_IF;".to_string()),
        (
            0,
            "// The writing end goes once no write is under way.".to_string(),
        ),
        (0, "IF CLOSE AND NOT closing THEN".to_string()),
        (1, "shut := TRUE;".to_string()),
        (0, "END_IF;".to_string()),
        (0, "closing := CLOSE;".to_string()),
        (0, "IF shut AND NOT BUSY THEN".to_string()),
        (1, "IF writable <> 0 THEN".to_string()),
        (2, format!("{drop}(stream := writable);")),
        (2, "writable := 0;".to_string()),
        (1, "END_IF;".to_string()),
        (1, "closed := TRUE;".to_string()),
        (1, "shut := FALSE;".to_string()),
        (0, "END_IF;".to_string()),
    ];
    let depth = out.depth + 1;
    for (indent, line) in body {
        out.depth = depth + indent;
        out.line(&line);
    }
    out.depth = depth - 1;
    out.line("END_FUNCTION_BLOCK");
    out.line("");
}

/// The POU `function` is declared as: a function by its name, a resource's
/// by the resource's name and its own, `DESCRIPTOR_READ_VIA_STREAM`, its
/// constructor as `<resource>_NEW`. `None` for a property, which is not
/// imported.
fn pou_name(resolve: &Resolve, function: &Function) -> Option<String> {
    let resource =
        |resource: &wit_parser::TypeId| resolve.types[*resource].name.clone().unwrap_or_default();
    match &function.kind {
        FunctionKind::Freestanding | FunctionKind::AsyncFreestanding => {
            Some(names::upper(&function.name))
        }
        FunctionKind::Method(id)
        | FunctionKind::AsyncMethod(id)
        | FunctionKind::Static(id)
        | FunctionKind::AsyncStatic(id) => Some(names::upper(&format!(
            "{}-{}",
            resource(id),
            function.item_name()
        ))),
        FunctionKind::Constructor(id) => Some(names::upper(&format!("{}-new", resource(id)))),
        _ => None,
    }
}

/// Records that the ST name `name` stands for `what` in a namespace, which
/// another item cannot share: ST compares names without case, and two WIT
/// names may be written alike, `x-new` and the constructor of `x`. `Err` is
/// what holds the name already.
fn claim(taken: &mut HashMap<String, String>, name: &str, what: &str) -> Result<(), String> {
    match taken.entry(name.to_ascii_uppercase()) {
        Entry::Occupied(other) => Err(other.get().clone()),
        Entry::Vacant(free) => {
            free.insert(what.to_string());
            Ok(())
        }
    }
}

/// `name`, a name the generator makes, with a `_` after it for as long as
/// the namespace holds it already: the names WIT gives come first.
fn spare(taken: &HashMap<String, String>, name: String) -> String {
    let mut name = name;
    while taken.contains_key(&name.to_ascii_uppercase()) {
        name.push('_');
    }
    name
}

/// [`spare`], and the namespace holds the name for `what` from here on.
fn own(taken: &mut HashMap<String, String>, name: String, what: &str) -> String {
    let name = spare(taken, name);
    taken.insert(name.to_ascii_uppercase(), what.to_string());
    name
}

/// Whether what `ty` names is passed without a type of its own: a string,
/// or value by value, an option or a result.
fn passed(resolve: &Resolve, ty: Type) -> bool {
    match shapes::resolved(resolve, ty) {
        Type::String => true,
        Type::Id(_) => matches!(
            shapes::definition(resolve, ty),
            Some(TypeDefKind::Option(_) | TypeDefKind::Result(_))
        ),
        _ => false,
    }
}

/// A block's `DONE`.
fn done() -> Var {
    Var {
        name: "DONE".to_string(),
        ty: "BOOL".to_string(),
        flat: 1,
        start: None,
    }
}

/// How wide a line of the generator's own comments is, past its `/* `.
const WIDTH: usize = 72;

/// `text` with each of its lines broken between words, so that none is
/// longer than `width` where a word allows; a line broken off starts with
/// `hang`.
fn wrap(text: &str, width: usize, hang: &str) -> String {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        let mut fresh = true;
        for word in paragraph.split_whitespace() {
            if !fresh && line.chars().count() + 1 + word.chars().count() > width {
                lines.push(std::mem::replace(&mut line, hang.to_string()));
                fresh = true;
            }
            if !fresh {
                line.push(' ');
            }
            line.push_str(word);
            fresh = false;
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// The comment each generated file starts with, as [`GENERATED`] says.
fn header(packages: &[String]) -> String {
    let mut out = Out::default();
    out.prose(&format!(
        "Generated from the WIT of {}. DO NOT EDIT!",
        packages.join(", ")
    ));
    out.text
}

/// The comment a file ends with, listing what was left out of it.
fn left_out(skipped: &[Skipped]) -> String {
    if skipped.is_empty() {
        return String::new();
    }
    let mut text = String::from("Not generated:");
    for skipped in skipped {
        text.push('\n');
        text.push_str(&wrap(
            &format!("- {}: {}.", skipped.item, skipped.reason),
            WIDTH,
            "  ",
        ));
    }
    let mut out = Out::default();
    out.comment(&text);
    out.text
}

/// What tells two streams' or futures' types apart: the type, through its
/// aliases, written out, a named type by its place in the resolve.
fn type_key(resolve: &Resolve, ty: Option<Type>) -> String {
    let Some(ty) = ty else {
        return "_".to_string();
    };
    match ty {
        Type::Id(id) => {
            let def = &resolve.types[id];
            match &def.kind {
                TypeDefKind::Type(inner) => type_key(resolve, Some(*inner)),
                TypeDefKind::Option(inner) => {
                    format!("option<{}>", type_key(resolve, Some(*inner)))
                }
                TypeDefKind::Result(result) => format!(
                    "result<{}, {}>",
                    type_key(resolve, result.ok),
                    type_key(resolve, result.err)
                ),
                TypeDefKind::Tuple(tuple) => format!(
                    "tuple<{}>",
                    tuple
                        .types
                        .iter()
                        .map(|ty| type_key(resolve, Some(*ty)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                TypeDefKind::List(inner) => format!("list<{}>", type_key(resolve, Some(*inner))),
                TypeDefKind::Handle(handle) => {
                    let (kind, resource) = match handle {
                        wit_parser::Handle::Own(resource) => ("own", resource),
                        wit_parser::Handle::Borrow(resource) => ("borrow", resource),
                    };
                    format!("{kind}<{}>", type_key(resolve, Some(Type::Id(*resource))))
                }
                _ => format!(
                    "{}#{}",
                    def.name.as_deref().unwrap_or("anonymous"),
                    id.index()
                ),
            }
        }
        primitive => wit_name(primitive).to_string(),
    }
}

/// `ty` as WIT writes it, a named type by its name: `result<_, error-code>`.
fn wit_text(resolve: &Resolve, ty: Option<Type>) -> String {
    let Some(ty) = ty else {
        return "_".to_string();
    };
    let Type::Id(id) = ty else {
        return wit_name(ty).to_string();
    };
    let def = &resolve.types[id];
    if let Some(name) = &def.name {
        return name.clone();
    }
    let list = |types: &[Type]| {
        types
            .iter()
            .map(|ty| wit_text(resolve, Some(*ty)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match &def.kind {
        TypeDefKind::Type(inner) => wit_text(resolve, Some(*inner)),
        TypeDefKind::Option(inner) => format!("option<{}>", wit_text(resolve, Some(*inner))),
        TypeDefKind::Result(result) => match (result.ok, result.err) {
            (None, None) => "result".to_string(),
            (ok, None) => format!("result<{}>", wit_text(resolve, ok)),
            (ok, err) => format!(
                "result<{}, {}>",
                wit_text(resolve, ok),
                wit_text(resolve, err)
            ),
        },
        TypeDefKind::Tuple(tuple) => format!("tuple<{}>", list(&tuple.types)),
        TypeDefKind::List(inner) => format!("list<{}>", wit_text(resolve, Some(*inner))),
        TypeDefKind::Handle(wit_parser::Handle::Own(resource)) => {
            format!("own<{}>", wit_text(resolve, Some(Type::Id(*resource))))
        }
        TypeDefKind::Handle(wit_parser::Handle::Borrow(resource)) => {
            format!("borrow<{}>", wit_text(resolve, Some(Type::Id(*resource))))
        }
        TypeDefKind::Future(payload) => match payload {
            Some(_) => format!("future<{}>", wit_text(resolve, *payload)),
            None => "future".to_string(),
        },
        TypeDefKind::Stream(payload) => match payload {
            Some(_) => format!("stream<{}>", wit_text(resolve, *payload)),
            None => "stream".to_string(),
        },
        _ => "type".to_string(),
    }
}

/// A primitive type's name in WIT.
fn wit_name(ty: Type) -> &'static str {
    match ty {
        Type::Bool => "bool",
        Type::U8 => "u8",
        Type::U16 => "u16",
        Type::U32 => "u32",
        Type::U64 => "u64",
        Type::S8 => "s8",
        Type::S16 => "s16",
        Type::S32 => "s32",
        Type::S64 => "s64",
        Type::F32 => "f32",
        Type::F64 => "f64",
        Type::Char => "char",
        Type::String => "string",
        Type::ErrorContext => "error-context",
        Type::Id(_) => "type",
    }
}

/// The directions a stream or a future of `key` crosses in the functions
/// of `owner`'s interface: from the program, which one of them takes as a
/// parameter, and to it, which one gives in its result.
fn directions(
    resolve: &Resolve,
    owner: Option<TypeOwner>,
    carrier: Carrier,
    key: &str,
) -> (bool, bool) {
    let Some(TypeOwner::Interface(interface)) = owner else {
        return (false, false);
    };
    let holds = |function: &Function| {
        function
            .find_futures_and_streams(resolve)
            .into_iter()
            .any(|ty| match &resolve.types[ty].kind {
                TypeDefKind::Stream(payload) => {
                    carrier == Carrier::Stream && type_key(resolve, *payload) == key
                }
                TypeDefKind::Future(payload) => {
                    carrier == Carrier::Future && type_key(resolve, *payload) == key
                }
                _ => false,
            })
    };
    let functions = &resolve.interfaces[interface].functions;
    let fed = functions.values().any(|function| {
        let mut params = function.clone();
        params.result = None;
        holds(&params)
    });
    let given = functions.values().any(|function| {
        let mut result = function.clone();
        result.params.clear();
        holds(&result)
    });
    (fed, given)
}

/// The name of the built-ins of a stream or a future of `payload`: a
/// stream of bytes is `BYTES`, a named type is its own name, a result is
/// `RESULT`, and its ok type after it, `RESULT_U64`.
fn set_name(resolve: &Resolve, carrier: Carrier, payload: Option<Type>) -> String {
    let Some(ty) = payload else {
        return "UNIT".to_string();
    };
    let ty = shapes::through_uses(resolve, ty);
    match ty {
        Type::U8 if carrier == Carrier::Stream => "BYTES".to_string(),
        Type::Id(id) => {
            let def = &resolve.types[id];
            match &def.kind {
                TypeDefKind::Type(inner) if def.name.is_none() => {
                    set_name(resolve, carrier, Some(*inner))
                }
                TypeDefKind::Handle(
                    wit_parser::Handle::Own(resource) | wit_parser::Handle::Borrow(resource),
                ) => capitals(resolve.types[*resource].name.as_deref().unwrap_or("handle")),
                TypeDefKind::Result(result) => {
                    let mut name = "RESULT".to_string();
                    if let Some(ok) = result.ok {
                        name.push('_');
                        name.push_str(&set_name(resolve, Carrier::Future, Some(ok)));
                    }
                    name
                }
                TypeDefKind::Option(inner) => {
                    format!(
                        "OPTION_{}",
                        set_name(resolve, Carrier::Future, Some(*inner))
                    )
                }
                TypeDefKind::List(inner) => {
                    format!("LIST_{}", set_name(resolve, Carrier::Future, Some(*inner)))
                }
                TypeDefKind::Tuple(tuple) => format!(
                    "TUPLE_{}",
                    tuple
                        .types
                        .iter()
                        .map(|ty| set_name(resolve, Carrier::Future, Some(*ty)))
                        .collect::<Vec<_>>()
                        .join("_")
                ),
                _ => capitals(def.name.as_deref().unwrap_or("anonymous")),
            }
        }
        primitive => capitals(wit_name(primitive)),
    }
}

/// `name` in capitals, its words joined by `_`, as part of a longer name:
/// no keyword escape, which a whole name takes.
fn capitals(name: &str) -> String {
    name.to_ascii_uppercase().replace('-', "_")
}

/// The built-ins of a stream's or a future's type, `name`, each imported as
/// `import` says with the built-in's name in place of its `{}`, written to
/// `out`, what is left out of them to `skipped`; the types they name, from
/// `owner`. `converts` is set when a future's value is read out of words.
/// `fed` says whether a function of the interface takes one from the
/// program, which the built-ins that make and write one are for, and
/// `given` whether one gives one to it, which those that read one are for.
/// The names of the POUs written: `name` and the built-in's own, with a
/// `_` after it where `taken` holds that already.
#[allow(clippy::too_many_arguments)]
fn built_in_set(
    resolve: &Resolve,
    sizes: &SizeAlign,
    out: &mut Out,
    skipped: &mut Vec<Skipped>,
    converts: &mut bool,
    taken: &HashMap<String, String>,
    carrier: Carrier,
    name: &str,
    payload: Option<Type>,
    import: &str,
    owner: Option<TypeOwner>,
    fed: bool,
    given: bool,
) -> (Vec<String>, Managers) {
    let mut written = Vec::new();
    let pou = |built_in: &str| spare(taken, format!("{name}_{built_in}"));
    let pragma = |built_in: &str| format!("'{}'", import.replace("{}", built_in));
    let end = carrier.prefix();
    let handle = |name: &str| Var {
        name: name.to_string(),
        ty: "UDINT".to_string(),
        flat: 1,
        start: None,
    };
    let kind = match carrier {
        Carrier::Stream => "stream",
        Carrier::Future => "future",
    };
    let of = match payload {
        Some(ty) => format!("`{}`", wit_text(resolve, Some(ty))),
        None => "nothing".to_string(),
    };

    if fed {
        out.prose(&format!(
            "Makes a {kind} of {of}, and gives its reading end, then its writing end."
        ));
        out.line(&format!("{{extern {}}}", pragma("new")));
        let new = pou("NEW");
        out.line(&format!("FUNCTION {new}"));
        written.push(new);
        out.vars("VAR_OUTPUT", &[handle("readable"), handle("writable")]);
        out.line("END_FUNCTION");
        out.line("");
    }

    let bytes = carrier == Carrier::Stream && payload == Some(Type::U8);
    let handles = match payload {
        Some(Type::Id(id)) => matches!(resolve.types[id].kind, TypeDefKind::Handle(_)),
        _ => false,
    };
    let closed = || Var {
        name: "closed".to_string(),
        ty: "BOOL".to_string(),
        flat: 1,
        start: None,
    };
    // A future's read: its outputs past its `DONE`, which `_GET` gives too.
    let mut read_outputs: Option<Vec<Var>> = None;
    match carrier {
        Carrier::Stream if bytes => {
            let reads = [
                ("READ", format!("STRING[{STRING_LIMIT}]")),
                (
                    "READ_ARRAY",
                    format!("ARRAY[0..{}] OF BYTE", BYTES_AT_ONCE - 1),
                ),
            ];
            for (block, data) in reads.into_iter().filter(|_| given) {
                out.prose(
                    "Reads the bytes that come next into `data`, as many as it holds at most: `read` of them, after `DONE`. `closed` is TRUE once the writer has let go.",
                );
                out.line(&format!("{{extern async {}}}", pragma("read")));
                let read = pou(block);
                out.line(&format!("FUNCTION_BLOCK {read}"));
                written.push(read);
                out.vars("VAR_INPUT", &[handle(end)]);
                out.vars(
                    "VAR_OUTPUT",
                    &[
                        done(),
                        Var {
                            name: "data".to_string(),
                            ty: data,
                            flat: 1,
                            start: None,
                        },
                        handle("read"),
                        closed(),
                    ],
                );
                out.line("END_FUNCTION_BLOCK");
                out.line("");
            }
            if fed {
                out.prose(
                    "Writes the bytes of `data`, as many as the reader takes: `written` of them, after `DONE`. `closed` is TRUE once the reader has let go.",
                );
                out.line(&format!("{{extern async {}}}", pragma("write")));
                let write = pou("WRITE");
                out.line(&format!("FUNCTION_BLOCK {write}"));
                written.push(write);
                out.vars(
                    "VAR_INPUT",
                    &[
                        handle(end),
                        Var {
                            name: "data".to_string(),
                            ty: format!("STRING[{STRING_LIMIT}]"),
                            flat: 2,
                            start: None,
                        },
                    ],
                );
                out.vars("VAR_OUTPUT", &[done(), handle("written"), closed()]);
                out.line("END_FUNCTION_BLOCK");
                out.line("");
                // Bytes that are no text, from where they are: the address of
                // the first, as the canonical ABI passes a buffer, and how many.
                out.prose(
                    "Writes the `count` bytes from the one `data` points at, as many as the reader takes: `written` of them, after `DONE`. The host reads them where they are while the write runs, so they stay there, unchanged, until `DONE`. `closed` is TRUE once the reader has let go.",
                );
                out.line(&format!("{{extern async {}}}", pragma("write")));
                let write = pou("WRITE_ARRAY");
                out.line(&format!("FUNCTION_BLOCK {write}"));
                written.push(write);
                out.vars(
                    "VAR_INPUT",
                    &[
                        handle(end),
                        Var {
                            name: "data".to_string(),
                            ty: "REF_TO BYTE".to_string(),
                            flat: 1,
                            start: None,
                        },
                        handle("count"),
                    ],
                );
                out.vars("VAR_OUTPUT", &[done(), handle("written"), closed()]);
                out.line("END_FUNCTION_BLOCK");
                out.line("");
            }
        }
        // A resource's handles, read into an array of its type, a UDINT:
        // what a stream of them is for, a listener's connections. Nothing a
        // component imports takes such a stream from it, which a write would
        // feed.
        Carrier::Stream if handles => {
            // Each a handle of the resource, as its type names it.
            let element = payload
                .and_then(|ty| shapes::scalar(resolve, ty, owner))
                .unwrap_or_else(|| "UDINT".to_string());
            if given {
                out.prose(
                "Reads the handles that come next into `data`, as many as it holds at most: `read` of them, after `DONE`, each the program's own to drop. `closed` is TRUE once the writer has let go.",
            );
                out.line(&format!("{{extern async {}}}", pragma("read")));
                let read = pou("READ");
                out.line(&format!("FUNCTION_BLOCK {read}"));
                written.push(read);
                out.vars("VAR_INPUT", &[handle(end)]);
                out.vars(
                    "VAR_OUTPUT",
                    &[
                        done(),
                        Var {
                            name: "data".to_string(),
                            ty: format!("ARRAY[0..{}] OF {element}", HANDLES_AT_ONCE - 1),
                            flat: 1,
                            start: None,
                        },
                        handle("read"),
                        closed(),
                    ],
                );
                out.line("END_FUNCTION_BLOCK");
                out.line("");
            }
            if fed {
                skipped.push(Skipped {
                    item: format!("`{name}_WRITE`"),
                    reason: "a stream's write moves bytes only".to_string(),
                });
            }
        }
        Carrier::Stream => {
            if given {
                skipped.push(Skipped {
                    item: format!("`{name}_READ`"),
                    reason: "a stream's read moves bytes or handles only".to_string(),
                });
            }
            if fed {
                skipped.push(Skipped {
                    item: format!("`{name}_WRITE`"),
                    reason: "a stream's write moves bytes only".to_string(),
                });
            }
        }
        Carrier::Future if given => {
            let reads = "Reads the value the future holds, which comes back after `DONE`.";
            // A value no record of outputs is laid out as comes back in
            // words, which the block reads it out of.
            // A value's field keeps away from the block's `DONE`, and
            // from the `call` of the block that reads it out of words.
            let lifted = |what| match payload {
                Some(ty) => {
                    let scope = Scope::holding(&["DONE", "call"]);
                    lift::lift(resolve, sizes, ty, owner, &scope, false, None)
                }
                None => Err(what),
            };
            let direct = shapes::result(resolve, payload, true, owner, &Scope::holding(&["DONE"]));
            // A value of a STRUCT laid out as a record of scalars comes
            // back in the import's outputs, as a record does.
            let scalars = match (&direct, payload) {
                (Err(_), Some(ty)) => lift::record_of_scalars(resolve, ty, owner),
                _ => None,
            };
            match direct.map_err(lifted) {
                Ok(shape) => {
                    let mut outputs = vec![done()];
                    if let Shape::Outputs(fields) = shape {
                        outputs.extend(fields);
                    }
                    read_outputs = Some(outputs[1..].to_vec());
                    out.prose(reads);
                    out.line(&format!("{{extern async {}}}", pragma("read")));
                    let read = pou("READ");
                    out.line(&format!("FUNCTION_BLOCK {read}"));
                    written.push(read);
                    out.vars("VAR_INPUT", &[handle(end)]);
                    out.vars("VAR_OUTPUT", &outputs);
                    out.line("END_FUNCTION_BLOCK");
                    out.line("");
                }
                Err(Ok(lifted)) => {
                    let raw = pou("READ_RAW");
                    let read = pou("READ");
                    // The outputs keep clear of the block's names.
                    let copied = match scalars {
                        Some(scalars) => {
                            let held = ["DONE", end]
                                .into_iter()
                                .map(str::to_string)
                                .chain(lifted.outputs.iter().map(|var| var.name.clone()))
                                .collect();
                            lifted.as_outputs(scalars, held)
                        }
                        None => Vec::new(),
                    };
                    let value = lifted.outputs.first().map(|var| var.name.as_str());
                    out.prose(&match (copied.len(), value) {
                        (0, _) | (_, None) => format!(
                            "The future's read as the host gives it: its value in words of 8 bytes, as the canonical ABI lays it out, which `{read}` reads."
                        ),
                        (1, Some(value)) => format!(
                            "The future's read as the host gives it: the one value the future holds, which `{read}` puts in the member of `{value}`."
                        ),
                        (_, Some(value)) => format!(
                            "The future's read as the host gives it: the values the future holds, which `{read}` puts in the members of `{value}`."
                        ),
                    });
                    let given: Vec<Var> = match copied.is_empty() {
                        true => lifted.words(1),
                        false => copied.iter().map(|(output, _)| output.clone()).collect(),
                    };
                    out.line(&format!("{{extern async {}}}", pragma("read")));
                    out.line(&format!("FUNCTION_BLOCK {raw}"));
                    out.vars("VAR_INPUT", &[handle(end)]);
                    out.vars("VAR_OUTPUT", &[vec![done()], given].concat());
                    out.line("END_FUNCTION_BLOCK");
                    out.line("");
                    out.prose(reads);
                    out.line(&format!("FUNCTION_BLOCK {read}"));
                    out.vars("VAR_INPUT", &[handle(end)]);
                    reader(out, &raw, &format!("{end} := {end}"), &lifted, &copied);
                    out.line("END_FUNCTION_BLOCK");
                    out.line("");
                    read_outputs = Some(lifted.outputs.clone());
                    written.push(raw);
                    written.push(read);
                    *converts |= copied.is_empty() && lifted.converts();
                }
                Err(Err(what)) => skipped.push(Skipped {
                    item: format!("`{name}_READ`"),
                    reason: format!("its value holds {what}, which a read cannot give back yet"),
                }),
            }
        }
        Carrier::Future => {}
    }
    if carrier == Carrier::Future && fed {
        skipped.push(Skipped {
            item: format!("`{name}_WRITE`"),
            reason: "a future's write is not imported yet".to_string(),
        });
    }
    let drops = [
        ("drop-readable", "READABLE", true),
        ("drop-writable", "WRITABLE", fed),
    ];
    for (built_in, which, _) in drops.into_iter().filter(|(.., wanted)| *wanted) {
        let side = if which == "READABLE" {
            "reading"
        } else {
            "writing"
        };
        out.prose(&format!("Lets go of the {kind}'s {side} end, `{end}`."));
        out.line(&format!("{{extern {}}}", pragma(built_in)));
        let drop = pou(&format!("DROP_{which}"));
        out.line(&format!("FUNCTION {drop}"));
        written.push(drop);
        out.vars("VAR_INPUT", &[handle(end)]);
        out.line("END_FUNCTION");
        out.line("");
    }
    let mut managers = Managers {
        writer: None,
        get: None,
        drop_readable: pou("DROP_READABLE"),
        read: (bytes && given).then(|| pou("READ")),
        array_writer: None,
        read_array: (bytes && given).then(|| pou("READ_ARRAY")),
    };
    if let Some(outputs) = read_outputs {
        let get = pou("GET");
        future_get(
            out,
            &get,
            &pou("READ"),
            &pou("DROP_READABLE"),
            end,
            &outputs,
        );
        written.push(get.clone());
        managers.get = Some((get, outputs));
    }
    // A writer hands its stream's reading end to a function of the
    // interface: there is none to take it unless one is `fed`.
    if bytes && fed {
        let names = Writer {
            writer: pou("WRITER"),
            len: pou("TEXT_LEN"),
            after: pou("TEXT_AFTER"),
            new: pou("NEW"),
            write: pou("WRITE"),
            drop: pou("DROP_WRITABLE"),
        };
        stream_writer(out, &names);
        managers.writer = Some(names.writer.clone());
        written.extend([names.len, names.after, names.writer]);
        let names = ArrayWriter {
            writer: pou("ARRAY_WRITER"),
            first: pou("ARRAY_FIRST"),
            len: pou("ARRAY_LEN"),
            new: pou("NEW"),
            write: pou("WRITE_ARRAY"),
            drop: pou("DROP_WRITABLE"),
        };
        stream_array_writer(out, &names);
        managers.array_writer = Some(names.writer.clone());
        written.extend([names.first, names.len, names.writer]);
    }
    let cancels: Vec<String> = [("READ", given), ("WRITE", fed)]
        .into_iter()
        .filter(|(_, used)| *used)
        .map(|(side, _)| format!("`{name}_CANCEL_{side}`"))
        .collect();
    if !cancels.is_empty() {
        skipped.push(Skipped {
            item: cancels.join(" and "),
            reason: "a cancel blocks the scan".to_string(),
        });
    }
    (written, managers)
}

/// The blocks of a set that manage an end for the program, which a
/// function's writer holds: the stream's `BYTES_STREAM_WRITER`, the
/// future's `_GET` with its outputs past `DONE`, and the drop of a future's
/// reading end, for a future no read gives back.
#[derive(Clone)]
struct Managers {
    writer: Option<String>,
    get: Option<(String, Vec<Var>)>,
    drop_readable: String,
    /// A stream of bytes' read into a STRING, which a reader reads with.
    read: Option<String>,
    /// A stream of bytes' writer from an array, and its read into one.
    array_writer: Option<String>,
    read_array: Option<String>,
}
