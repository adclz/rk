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

/// The world `world` of the package in the WIT folder `dir`, laid out as
/// WIT's tools lay one out: the package's files, and in `deps/` the
/// packages it uses. `None` takes the package's one world.
pub fn resolve_folder(dir: &Path, world: Option<&str>) -> Result<(Resolve, WorldId), String> {
    let mut resolve = Resolve {
        // A feature a package has not released yet is generated too, as
        // WASI's time zone is.
        all_features: true,
        ..Default::default()
    };
    let (package, _) = resolve
        .push_dir(dir)
        .map_err(|e| format!("reading the WIT in `{}`: {e:#}", dir.display()))?;
    let world = resolve
        .select_world(&[package], world)
        .map_err(|e| format!("selecting the world: {e:#}"))?;
    Ok((resolve, world))
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
            match var.start {
                Some(start) => self.line(&format!("{} : {} := {start};", var.name, var.ty)),
                None => self.line(&format!("{} : {};", var.name, var.ty)),
            }
        }
        self.depth -= 1;
        self.line("END_VAR");
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
        // The sets of built-ins of this interface's own types, by type.
        let mut own_sets: Vec<(Carrier, String)> = Vec::new();
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
                Ok(converted) => converts |= converted,
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
                converts: &mut converts,
                owner: here,
            };
            self.built_ins(&module, function, &mut here_out);
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
    ) -> Result<bool, String> {
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
        if !apart {
            return Ok(false);
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
        Ok(lowered.converts() || matches!(&returned, Returned::Words(lifted) if lifted.converts()))
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
            let fed = taken_from_the_program(resolve, here.owner, carrier, &key.1);
            here.sets.push(key);
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
            let written = built_in_set(
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
            );
            let what = format!("a built-in of `{}`", wit_text(resolve, payload));
            for pou in written {
                here.taken.insert(pou.to_ascii_uppercase(), what.clone());
            }
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

/// Whether a function of the interface `owner` takes a stream or a future,
/// as `carrier` says, of the payload `key` names from the program: one of
/// its parameters holds one, which a write of the program's would feed.
fn taken_from_the_program(
    resolve: &Resolve,
    owner: Option<TypeOwner>,
    carrier: Carrier,
    key: &str,
) -> bool {
    let Some(TypeOwner::Interface(interface)) = owner else {
        return false;
    };
    resolve.interfaces[interface]
        .functions
        .values()
        .any(|function| {
            let mut params = function.clone();
            params.result = None;
            params
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
        })
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
/// program, without which a write would have no reader.
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
) -> Vec<String> {
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
    match carrier {
        Carrier::Stream if bytes => {
            for (block, data) in [
                ("READ", format!("STRING[{STRING_LIMIT}]")),
                (
                    "READ_ARRAY",
                    format!("ARRAY[0..{}] OF BYTE", BYTES_AT_ONCE - 1),
                ),
            ] {
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
        // A resource's handles, read into an array of its type, a UDINT:
        // what a stream of them is for, a listener's connections. Nothing a
        // component imports takes such a stream from it, which a write would
        // feed.
        Carrier::Stream if handles => {
            // Each a handle of the resource, as its type names it.
            let element = payload
                .and_then(|ty| shapes::scalar(resolve, ty, owner))
                .unwrap_or_else(|| "UDINT".to_string());
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
            skipped.push(Skipped {
                item: format!("`{name}_WRITE`"),
                reason: match fed {
                    true => "a stream's write moves bytes only".to_string(),
                    false => format!(
                        "no function of the interface takes a stream of {of} from the program"
                    ),
                },
            });
        }
        Carrier::Stream => skipped.push(Skipped {
            item: format!("`{name}_READ` and `{name}_WRITE`"),
            reason: "a stream's read moves bytes or handles only, and its write bytes only"
                .to_string(),
        }),
        Carrier::Future => {
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
                    written.push(raw);
                    written.push(read);
                    *converts |= copied.is_empty() && lifted.converts();
                }
                Err(Err(what)) => skipped.push(Skipped {
                    item: format!("`{name}_READ`"),
                    reason: format!("its value holds {what}, which a read cannot give back yet"),
                }),
            }
            skipped.push(Skipped {
                item: format!("`{name}_WRITE`"),
                reason: match (fed, payload) {
                    (true, _) => "a future's write is not imported yet".to_string(),
                    (false, Some(_)) => format!(
                        "no function of the interface takes a future of {of} from the program"
                    ),
                    (false, None) => {
                        "no function of the interface takes a future from the program".to_string()
                    }
                },
            });
        }
    }
    for (built_in, which) in [("drop-readable", "READABLE"), ("drop-writable", "WRITABLE")] {
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
    skipped.push(Skipped {
        item: format!("`{name}_CANCEL_READ` and `{name}_CANCEL_WRITE`"),
        reason: "a cancel blocks the scan".to_string(),
    });
    written
}
