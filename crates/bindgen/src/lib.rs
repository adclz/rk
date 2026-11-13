// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! ST declarations for the WIT interfaces a world imports, as wit-bindgen
//! writes Rust for them: a FUNCTION for each function, an `{extern async}`
//! FUNCTION_BLOCK for each async one, a typed enum for each enum, a TYPE for
//! each alias of a scalar, the drop of each resource, and the built-ins of
//! each stream's and future's type, once per type, under the name of the
//! first function whose type has it.
//!
//! Each interface is a namespace inside its package's, inside `Wasi`, the
//! one INTERNAL namespace: `wasi:clocks/monotonic-clock` is
//! `Wasi.Clocks.MonotonicClock`, which only the library that holds the
//! code reaches. The built-ins are in `Wasi.Streams` and `Wasi.Futures`. A
//! function or a type ST cannot express yet is left out, and the file says
//! why at its end.

pub mod names;
pub mod shapes;

use std::collections::HashMap;

use wit_parser::{
    Docs, Function, FunctionKind, InterfaceId, Resolve, Type, TypeDefKind, TypeOwner, WorldId,
    WorldItem,
};

use shapes::{STRING_LIMIT, Shape, Var};

/// The interfaces `Std` reaches WASI through, as a world for [`resolve`].
pub const STD_WORLD: &str = "package rk:std-wasi;

world std {
    import wasi:cli/stdout@0.3.0;
    import wasi:cli/stderr@0.3.0;
    import wasi:clocks/monotonic-clock@0.3.0;
    import wasi:clocks/system-clock@0.3.0;
    import wasi:clocks/timezone@0.3.0;
}
";

/// The canonical ABI passes a function's parameters as values up to this
/// many, and in memory past it, which an import declared in ST does not.
const MAX_FLAT_PARAMS: usize = 16;

/// An async call takes its parameters as values up to this many, and as
/// the address of a tuple of them past it, which rk lays out field by field.
const MAX_FLAT_ASYNC_PARAMS: usize = 4;

/// A file of generated code.
#[derive(Debug)]
pub struct File {
    /// Its name, under `stdlib/generated/`: `wasi-clocks-monotonic-clock.st`.
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

/// The world in `world`, a WIT package with one world, resolved against the
/// WASI packages rk carries, whose interfaces it may import.
pub fn resolve(world: &str) -> Result<(Resolve, WorldId), String> {
    let mut resolve = Resolve {
        // WASI's time zone is `@unstable(feature = clocks-timezone)`.
        all_features: true,
        ..Default::default()
    };
    let deps = debug_format::wit::WASI
        .iter()
        .map(|package| parse(package.dir, package.files))
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
/// functions take or give.
pub fn generate(resolve: &Resolve, world: WorldId) -> Result<Vec<File>, String> {
    let mut generator = Generator {
        resolve,
        files: Vec::new(),
        streams: Section::default(),
        futures: Section::default(),
        sets: HashMap::new(),
        set_names: HashMap::new(),
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
    generator.finish()
}

/// The text of a file under way, indented as deep as its namespaces.
#[derive(Default)]
struct Out {
    text: String,
    depth: usize,
}

impl Out {
    fn line(&mut self, line: &str) {
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
            self.line(&format!("{} : {};", var.name, var.ty));
        }
        self.depth -= 1;
        self.line("END_VAR");
    }
}

/// The streams' or the futures' built-ins, and what was left out of them.
#[derive(Default)]
struct Section {
    out: Out,
    skipped: Vec<Skipped>,
    /// The packages whose WIT the built-ins come from.
    packages: Vec<String>,
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

struct Generator<'a> {
    resolve: &'a Resolve,
    files: Vec<File>,
    streams: Section,
    futures: Section,
    /// Each set of built-ins generated, by its type: its name.
    sets: HashMap<(Carrier, String), String>,
    /// Each set's name, by the type it serves: two types may not share one.
    set_names: HashMap<(Carrier, String), String>,
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

        let mut out = Out {
            text: String::new(),
            depth: 3,
        };
        let mut skipped = Vec::new();
        for (name, &ty) in &interface.types {
            let def = &resolve.types[ty];
            match &def.kind {
                // A `use`: the generated code names the type where it is defined.
                TypeDefKind::Type(Type::Id(other)) if resolve.types[*other].owner != def.owner => {}
                TypeDefKind::Type(inner) => match shapes::scalar(resolve, *inner, here) {
                    Some(st) => {
                        out.docs(&def.docs);
                        out.line(&format!("TYPE {} : {st}; END_TYPE", names::upper(name)));
                        out.line("");
                    }
                    None => skipped.push(Skipped {
                        item: format!("`{name}`"),
                        reason: format!(
                            "it names {}, which ST has no scalar for",
                            shapes::what(resolve, *inner)
                        ),
                    }),
                },
                TypeDefKind::Enum(enumeration) => {
                    let base = match enumeration.cases.len() {
                        0..=256 => "USINT",
                        257..=65536 => "UINT",
                        _ => "UDINT",
                    };
                    let cases: Vec<String> = enumeration
                        .cases
                        .iter()
                        .enumerate()
                        .map(|(at, case)| format!("{} := {at}", names::upper(&case.name)))
                        .collect();
                    out.docs(&def.docs);
                    out.line(&format!("TYPE {} : {base} (", names::upper(name)));
                    out.depth += 1;
                    let last = cases.len().saturating_sub(1);
                    for (at, case) in cases.iter().enumerate() {
                        out.line(&format!("{case}{}", if at == last { "" } else { "," }));
                    }
                    out.depth -= 1;
                    out.line("); END_TYPE");
                    out.line("");
                }
                TypeDefKind::Resource => {
                    out.prose(&format!(
                        "Lets go of a `{name}` the host gave, whose handle is `handle`."
                    ));
                    out.line(&format!("{{extern '{module}' '[resource-drop]{name}'}}"));
                    out.line(&format!("FUNCTION {}_DROP", names::upper(name)));
                    out.vars(
                        "VAR_INPUT",
                        &[Var {
                            name: "handle".to_string(),
                            ty: "UDINT".to_string(),
                            flat: 1,
                        }],
                    );
                    out.line("END_FUNCTION");
                    out.line("");
                }
                // Passed field by field, or a handle, which is a UDINT.
                TypeDefKind::Record(_)
                | TypeDefKind::Tuple(_)
                | TypeDefKind::Option(_)
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
        for function in interface.functions.values() {
            if let Err(reason) = self.function(&mut out, &module, function, here) {
                skipped.push(Skipped {
                    item: format!("`{}`", function.name),
                    reason,
                });
            }
            self.built_ins(&module, &package.to_string(), function)?;
        }

        let mut text = header(&[package.to_string()]);
        if !out.text.is_empty() {
            let mut wrapper = Out::default();
            wrapper.line("NAMESPACE INTERNAL Wasi");
            wrapper.depth += 1;
            wrapper.line(&format!("NAMESPACE {}", names::namespace(&package.name)));
            wrapper.depth += 1;
            wrapper.docs(&interface.docs);
            wrapper.line(&format!("NAMESPACE {}", names::namespace(interface_name)));
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
            name: format!("{}-{}-{interface_name}.st", package.namespace, package.name),
            text,
            skipped,
        });
        Ok(())
    }

    /// The declaration of `function`, imported from `module`, written to
    /// `out`; `Err` says why it cannot be.
    fn function(
        &self,
        out: &mut Out,
        module: &str,
        function: &Function,
        here: Option<TypeOwner>,
    ) -> Result<(), String> {
        let resolve = self.resolve;
        let pou = match &function.kind {
            FunctionKind::Freestanding | FunctionKind::AsyncFreestanding => {
                names::upper(&function.name)
            }
            FunctionKind::Method(resource)
            | FunctionKind::AsyncMethod(resource)
            | FunctionKind::Static(resource)
            | FunctionKind::AsyncStatic(resource) => {
                let resource = resolve.types[*resource].name.as_deref().unwrap_or_default();
                names::upper(&format!("{resource}-{}", function.item_name()))
            }
            FunctionKind::Constructor(resource) => {
                let resource = resolve.types[*resource].name.as_deref().unwrap_or_default();
                names::upper(&format!("{resource}-new"))
            }
            _ => return Err("it is a property, which rk does not import".to_string()),
        };
        let asynchronous = function.kind.is_async();
        let mut inputs = Vec::new();
        for param in &function.params {
            let vars = shapes::input(resolve, &param.name, param.ty, here).map_err(|what| {
                format!(
                    "its parameter `{}` holds {what}, which an import cannot take yet",
                    param.name
                )
            })?;
            inputs.extend(vars);
        }
        let flat: usize = inputs.iter().map(|var| var.flat).sum();
        if !asynchronous && flat > MAX_FLAT_PARAMS {
            return Err(format!(
                "it takes {flat} values, which the canonical ABI passes in memory past {MAX_FLAT_PARAMS}"
            ));
        }
        let flattened = function.params.iter().any(|param| {
            shapes::scalar(resolve, param.ty, here).is_none() && param.ty != Type::String
        });
        if asynchronous && flattened && flat > MAX_FLAT_ASYNC_PARAMS {
            return Err(format!(
                "it takes a record among {flat} values, which an async call passes in memory as a tuple holding the record whole"
            ));
        }
        let shape =
            shapes::result(resolve, function.result, asynchronous, here).map_err(|what| {
                format!("its result holds {what}, which an import cannot give back yet")
            })?;

        out.docs(&function.docs);
        let name = &function.name;
        match asynchronous {
            false => {
                out.line(&format!("{{extern '{module}' '{name}'}}"));
                match &shape {
                    Shape::Return(st) => out.line(&format!("FUNCTION {pou} : {st}")),
                    _ => out.line(&format!("FUNCTION {pou}")),
                }
                out.vars("VAR_INPUT", &inputs);
                if let Shape::Outputs(outputs) = &shape {
                    out.vars("VAR_OUTPUT", outputs);
                }
                out.line("END_FUNCTION");
            }
            true => {
                out.line(&format!("{{extern async '{module}' '{name}'}}"));
                out.line(&format!("FUNCTION_BLOCK {pou}"));
                out.vars("VAR_INPUT", &inputs);
                let mut outputs = vec![done()];
                if let Shape::Outputs(fields) = shape {
                    outputs.extend(fields);
                }
                out.vars("VAR_OUTPUT", &outputs);
                out.line("END_FUNCTION_BLOCK");
            }
        }
        out.line("");
        Ok(())
    }

    /// The built-ins of each stream and future `function` takes or gives
    /// whose type has none yet, imported from `module` under its name.
    fn built_ins(
        &mut self,
        module: &str,
        package: &str,
        function: &Function,
    ) -> Result<(), String> {
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
            if self.sets.contains_key(&key) {
                continue;
            }
            let name = set_name(resolve, carrier, payload);
            if let Some(other) = self
                .set_names
                .insert((carrier, name.clone()), key.1.clone())
            {
                return Err(format!(
                    "two {}s' built-ins would be named `{name}`: of `{other}` and of `{}`",
                    carrier.prefix(),
                    key.1
                ));
            }
            self.sets.insert(key, name.clone());
            let section = match carrier {
                Carrier::Stream => &mut self.streams,
                Carrier::Future => &mut self.futures,
            };
            if !section.packages.iter().any(|known| known == package) {
                section.packages.push(package.to_string());
            }
            built_in_set(
                resolve,
                section,
                carrier,
                &name,
                payload,
                &format!(
                    "{module}' '[{}-{{}}-{index}]{}",
                    carrier.prefix(),
                    function.name
                ),
            );
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<File>, String> {
        for (carrier, section) in [
            (Carrier::Stream, std::mem::take(&mut self.streams)),
            (Carrier::Future, std::mem::take(&mut self.futures)),
        ] {
            if section.out.text.is_empty() && section.skipped.is_empty() {
                continue;
            }
            let namespace = match carrier {
                Carrier::Stream => "Streams",
                Carrier::Future => "Futures",
            };
            let mut text = header(&section.packages);
            if !section.out.text.is_empty() {
                text.push_str("NAMESPACE INTERNAL Wasi\n");
                text.push_str(&format!("\tNAMESPACE {namespace}\n"));
                text.push_str(section.out.text.trim_end_matches('\n'));
                text.push_str("\n\tEND_NAMESPACE\nEND_NAMESPACE\n");
            }
            text.push_str(&left_out(&section.skipped));
            self.files.push(File {
                name: format!("wasi-{}.st", namespace.to_ascii_lowercase()),
                text,
                skipped: section.skipped,
            });
        }
        Ok(self.files)
    }
}

/// A block's `DONE`.
fn done() -> Var {
    Var {
        name: "DONE".to_string(),
        ty: "BOOL".to_string(),
        flat: 1,
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

/// The comment each generated file starts with.
fn header(packages: &[String]) -> String {
    let mut out = Out::default();
    out.prose(&format!(
        "Generated by rk-bindgen from the WIT of {}. Do not edit: `cargo run -p rk-bindgen` writes it again.",
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

/// The name of the built-ins of a stream or a future of `payload`: a
/// stream of bytes is `BYTES`, a named type is its own name, a result is
/// named after the package of its error, `CLI_RESULT`.
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
                ) => names::upper(resolve.types[*resource].name.as_deref().unwrap_or("handle")),
                TypeDefKind::Result(result) => {
                    let package = [result.err, result.ok]
                        .into_iter()
                        .flatten()
                        .find_map(|ty| package_of(resolve, ty));
                    let mut name = match package {
                        Some(package) => format!("{}_RESULT", names::upper(&package)),
                        None => "RESULT".to_string(),
                    };
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
                _ => names::upper(def.name.as_deref().unwrap_or("anonymous")),
            }
        }
        primitive => names::upper(wit_name(primitive)),
    }
}

/// The name of the package that defines the type `ty` names, if any.
fn package_of(resolve: &Resolve, ty: Type) -> Option<String> {
    let Type::Id(id) = shapes::through_uses(resolve, ty) else {
        return None;
    };
    let TypeOwner::Interface(interface) = resolve.types[id].owner else {
        return None;
    };
    let package = resolve.interfaces[interface].package?;
    Some(resolve.packages[package].name.name.clone())
}

/// The built-ins of a stream's or a future's type, `name`, each imported as
/// `import` says with the built-in's name in place of its `{}`.
fn built_in_set(
    resolve: &Resolve,
    section: &mut Section,
    carrier: Carrier,
    name: &str,
    payload: Option<Type>,
    import: &str,
) {
    let out = &mut section.out;
    out.depth = 2;
    let pragma = |built_in: &str| format!("'{}'", import.replace("{}", built_in));
    let end = carrier.prefix();
    let handle = |name: &str| Var {
        name: name.to_string(),
        ty: "UDINT".to_string(),
        flat: 1,
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
    out.line(&format!("FUNCTION {name}_NEW"));
    out.vars("VAR_OUTPUT", &[handle("readable"), handle("writable")]);
    out.line("END_FUNCTION");
    out.line("");

    let bytes = carrier == Carrier::Stream && payload == Some(Type::U8);
    match carrier {
        Carrier::Stream if bytes => {
            for (block, data) in [
                ("READ", format!("STRING[{STRING_LIMIT}]")),
                (
                    "READ_ARRAY",
                    format!("ARRAY[0..{}] OF BYTE", STRING_LIMIT - 1),
                ),
            ] {
                out.prose(
                    "Reads the bytes that come next into `data`, as many as it holds at most: `read` of them, after `DONE`. `closed` is TRUE once the writer has let go.",
                );
                out.line(&format!("{{extern async {}}}", pragma("read")));
                out.line(&format!("FUNCTION_BLOCK {name}_{block}"));
                out.vars("VAR_INPUT", &[handle(end)]);
                out.vars(
                    "VAR_OUTPUT",
                    &[
                        done(),
                        Var {
                            name: "data".to_string(),
                            ty: data,
                            flat: 1,
                        },
                        handle("read"),
                        Var {
                            name: "closed".to_string(),
                            ty: "BOOL".to_string(),
                            flat: 1,
                        },
                    ],
                );
                out.line("END_FUNCTION_BLOCK");
                out.line("");
            }
            out.prose(
                "Writes the bytes of `data`, as many as the reader takes: `written` of them, after `DONE`. `closed` is TRUE once the reader has let go.",
            );
            out.line(&format!("{{extern async {}}}", pragma("write")));
            out.line(&format!("FUNCTION_BLOCK {name}_WRITE"));
            out.vars(
                "VAR_INPUT",
                &[
                    handle(end),
                    Var {
                        name: "data".to_string(),
                        ty: format!("STRING[{STRING_LIMIT}]"),
                        flat: 2,
                    },
                ],
            );
            out.vars(
                "VAR_OUTPUT",
                &[
                    done(),
                    handle("written"),
                    Var {
                        name: "closed".to_string(),
                        ty: "BOOL".to_string(),
                        flat: 1,
                    },
                ],
            );
            out.line("END_FUNCTION_BLOCK");
            out.line("");
        }
        Carrier::Stream => section.skipped.push(Skipped {
            item: format!("`{name}_READ` and `{name}_WRITE`"),
            reason: "a stream's read or write moves bytes only".to_string(),
        }),
        Carrier::Future => {
            match shapes::result(resolve, payload, true, None) {
                Ok(shape) => {
                    let mut outputs = vec![done()];
                    if let Shape::Outputs(fields) = shape {
                        outputs.extend(fields);
                    }
                    out.prose("Reads the value the future holds, which comes back after `DONE`.");
                    out.line(&format!("{{extern async {}}}", pragma("read")));
                    out.line(&format!("FUNCTION_BLOCK {name}_READ"));
                    out.vars("VAR_INPUT", &[handle(end)]);
                    out.vars("VAR_OUTPUT", &outputs);
                    out.line("END_FUNCTION_BLOCK");
                    out.line("");
                }
                Err(what) => section.skipped.push(Skipped {
                    item: format!("`{name}_READ`"),
                    reason: format!("its value holds {what}, which a read cannot give back yet"),
                }),
            }
            section.skipped.push(Skipped {
                item: format!("`{name}_WRITE`"),
                reason: "a future's write is not imported yet".to_string(),
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
        out.line(&format!("FUNCTION {name}_DROP_{which}"));
        out.vars("VAR_INPUT", &[handle(end)]);
        out.line("END_FUNCTION");
        out.line("");
    }
    section.skipped.push(Skipped {
        item: format!("`{name}_CANCEL_READ` and `{name}_CANCEL_WRITE`"),
        reason: "a cancel blocks the scan".to_string(),
    });
}
