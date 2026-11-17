// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Parameters the generated code packs itself, as wit-bindgen's does. The
//! canonical ABI passes a variant as its case, then values its cases
//! share, each as wide as the widest any case puts there; and in memory,
//! where an async call's parameters go past 4 values, a parameter that
//! holds other values is laid out whole, as its type is. Neither is a list
//! of inputs. The import takes words in their place, `a0`, `a1`, and the
//! code written here packs each value into them: a flag or a case first,
//! then what that case holds.
//!
//! The inputs a call gives are the values themselves: a value ST has a
//! STRUCT for is one input, whose members are packed (`target.kind`,
//! `target.circle`), and an option or a result no name holds is its flag
//! and what it holds, named as a result's outputs are. One that is only
//! passed with its flag starts at zero, so a call names only what it
//! passes. A STRUCT the canonical ABI passes as values, a record of
//! scalars or strings, is passed field by field instead, each field a
//! parameter of the import named after the input and the field
//! (`when_seconds := when.seconds`).
//!
//! A list of bytes passed in is the caller's `ARRAY[*] OF BYTE` and a
//! `count` of its first elements that go: the import takes what the
//! canonical ABI passes, the address of the elements, a `REF_TO BYTE`, and
//! their number, and the code written here checks that the count stays in
//! the array.

use wit_parser::abi::{FlatTypes, WasmType};
use wit_parser::{Function, Int, Resolve, SizeAlign, Type, TypeDefKind, TypeId, TypeOwner};

use crate::lift::{self, At, Kind, Step, held_by_option, join, kind, tag_kind};
use crate::names::{self, Scope};
use crate::shapes::{self, Var};
use crate::types;
use crate::{MAX_FLAT_ASYNC_PARAMS, Out};

/// A word of the import's parameters that values are packed into.
#[derive(Debug)]
struct Slot {
    name: String,
    /// Where it is: its offset in bytes among the parameters in memory, or
    /// its place among the values its parameter is passed as.
    at: usize,
    bytes: usize,
    /// A REAL or an LREAL, which takes its value as it is.
    float: bool,
}

impl Slot {
    fn ty(&self) -> &'static str {
        match (self.float, self.bytes) {
            (true, 4) => "REAL",
            (true, _) => "LREAL",
            (false, 1) => "BYTE",
            (false, 2) => "WORD",
            (false, 4) => "DWORD",
            (false, _) => "LWORD",
        }
    }

    fn var(&self) -> Var {
        Var {
            name: self.name.clone(),
            ty: self.ty().to_string(),
            flat: 1,
            start: None,
        }
    }

    /// The statement that packs `source`, held as `kind`, `within` bytes
    /// into the word: the part of the word the value is, as the bits it
    /// is.
    fn put(&self, within: usize, kind: Kind, source: &str) -> String {
        let name = &self.name;
        if self.float {
            assert_eq!(
                (kind, within),
                (Kind::Float(self.bytes), 0),
                "a value the cases agree on is passed as it is"
            );
            return format!("{name} := {source};");
        }
        if kind == Kind::Bool {
            return format!("{name}.%X{} := {source};", within * 8);
        }
        let bytes = kind.bytes();
        assert!(
            within.is_multiple_of(bytes) && within + bytes <= self.bytes,
            "a value lies whole in a word, where the canonical ABI aligns it"
        );
        let part = match bytes {
            _ if bytes == self.bytes => name.clone(),
            1 => format!("{name}.%B{within}"),
            2 => format!("{name}.%W{}", within / 2),
            _ => format!("{name}.%D{}", within / 4),
        };
        let converted =
            |names: [&str; 4]| format!("{}({source})", names[bytes.trailing_zeros() as usize]);
        let bits = match kind {
            Kind::Bool | Kind::Bits(_) => source.to_string(),
            Kind::Unsigned(_) => converted([
                "USINT_TO_BYTE",
                "UINT_TO_WORD",
                "UDINT_TO_DWORD",
                "ULINT_TO_LWORD",
            ]),
            Kind::Signed(_) => converted([
                "SINT_TO_BYTE",
                "INT_TO_WORD",
                "DINT_TO_DWORD",
                "LINT_TO_LWORD",
            ]),
            Kind::Float(_) => converted(["", "", "REAL_TO_DWORD", "LREAL_TO_LWORD"]),
        };
        format!("{part} := {bits};")
    }
}

/// A value to pack: the input it is, where the canonical ABI has it, in
/// bytes among the parameters in memory and in values after its
/// parameter's first, and how it is held.
#[derive(Debug)]
struct Put {
    source: String,
    offset: usize,
    position: usize,
    kind: Kind,
}

/// The input that says how many of the first elements of a list passed in
/// go: a `_` after a parameter WIT names so, as after any name the
/// generated code uses.
pub const LIST_COUNT: &str = "count";

/// A function's parameters, as a call gives them and as its import takes
/// them.
#[derive(Debug, Default)]
pub struct Lowered {
    /// The inputs a call gives: each value by its name. One that is only
    /// passed with its case starts at zero.
    pub inputs: Vec<Var>,
    /// The arrays a call gives, a list of bytes passed in each, whose
    /// elements the import reads in place.
    pub in_outs: Vec<Var>,
    /// The import's parameters, in order: an input passed as it is, or a
    /// word values are packed into.
    pub params: Vec<Var>,
    /// Where a call takes each parameter of the import from that is no
    /// input of its own: a member of a STRUCT the call is given, a field
    /// of a record passed as values.
    pub sources: Vec<(String, String)>,
    /// How many values the canonical ABI flattens the parameters into.
    pub flat: usize,
    slots: Vec<Slot>,
    steps: Vec<Step<String>>,
    /// Whether packing converts a value, with `Std.Convert`.
    converts: bool,
}

/// Whether `ty` is a list of bytes, through the aliases that name it.
fn byte_list(resolve: &Resolve, ty: Type) -> bool {
    let mut ty = shapes::through_uses(resolve, ty);
    while let Type::Id(id) = ty {
        match &resolve.types[id].kind {
            TypeDefKind::Type(inner) => ty = *inner,
            TypeDefKind::List(Type::U8) => return true,
            _ => return false,
        }
    }
    false
}

impl Lowered {
    /// Whether the code that packs the inputs converts a value, and needs
    /// `Std.Convert`.
    pub fn converts(&self) -> bool {
        self.converts
    }

    /// Whether values are packed into words: the import's parameters are
    /// then not the inputs a call gives.
    pub fn packs(&self) -> bool {
        !self.slots.is_empty()
    }

    /// Whether a list of bytes is passed in, from the caller's array.
    pub fn lists(&self) -> bool {
        !self.in_outs.is_empty()
    }

    /// Whether the import's parameters are not the inputs a call gives:
    /// values packed into words, or the fields of a STRUCT passed as
    /// values.
    pub fn apart(&self) -> bool {
        self.packs() || !self.sources.is_empty()
    }

    /// What a call passes for the import's parameter `name`: the member it
    /// is taken from, or the call's own input of that name.
    pub fn source<'a>(&'a self, name: &'a str) -> &'a str {
        self.sources
            .iter()
            .find(|(param, _)| param == name)
            .map_or(name, |(_, source)| source)
    }

    /// The words values are packed into.
    pub fn words(&self) -> Vec<Var> {
        self.slots.iter().map(Slot::var).collect()
    }

    /// Writes the statements that check a list's count against its array,
    /// then pack the inputs into the words: each word at zero first, then
    /// each value that is passed.
    pub(crate) fn write(&self, out: &mut Out) {
        for array in &self.in_outs {
            out.line(&format!(
                "IF {LIST_COUNT} > DINT_TO_UDINT(Std.Arrays.UPPER_BOUND({0}, 1) - Std.Arrays.LOWER_BOUND({0}, 1) + 1) THEN",
                array.name
            ));
            out.depth += 1;
            out.line("__RAISE('a list passed to the host is longer than its array');");
            out.depth -= 1;
            out.line("END_IF;");
        }
        for slot in &self.slots {
            let zero = if slot.float { "0.0" } else { "0" };
            out.line(&format!("{} := {zero};", slot.name));
        }
        lift::write(out, &self.steps, &|statement| statement.clone());
    }
}

/// `function`'s parameters as its import takes them: as values, or
/// `in_memory`, where the canonical ABI lays them out as a tuple, its
/// inputs named in `scope`. `Err` says which cannot be passed, and why.
pub fn lower(
    resolve: &Resolve,
    sizes: &SizeAlign,
    function: &Function,
    in_memory: bool,
    here: Option<TypeOwner>,
    scope: &Scope,
) -> Result<Lowered, String> {
    let mut lowered = Lowered::default();
    let types: Vec<Type> = function.params.iter().map(|param| param.ty).collect();
    // Where rk lays the import's parameters out in memory, one after the
    // other as a record's fields: each must land where the canonical ABI
    // has it.
    let mut laid = 0usize;
    let mut lay = |bytes: usize, align: usize, expected: Option<usize>| {
        laid = laid.next_multiple_of(align);
        if let Some(expected) = expected {
            assert_eq!(
                laid, expected,
                "rk lays a parameter out where the canonical ABI has it"
            );
        }
        laid += bytes;
    };
    for (param, (offset, _)) in function.params.iter().zip(sizes.field_offsets(&types)) {
        let offset = offset.size_wasm32();
        // A list of bytes: the address of the caller's elements, then how
        // many go, where the canonical ABI has a list's.
        if byte_list(resolve, param.ty) {
            if in_memory {
                lay(4, 4, Some(offset));
                lay(4, 4, Some(offset + 4));
            }
            lowered.flat += 2;
            let array = scope.variable(&param.name);
            let var = |name: &str, ty: &str| Var {
                name: name.to_string(),
                ty: ty.to_string(),
                flat: 1,
                start: None,
            };
            lowered.params.push(var(&array, "REF_TO BYTE"));
            lowered.params.push(var(LIST_COUNT, "UDINT"));
            lowered.sources.push((
                array.clone(),
                format!("REF({array}[Std.Arrays.LOWER_BOUND({array}, 1)])"),
            ));
            lowered.in_outs.push(var(&array, "ARRAY[*] OF BYTE"));
            lowered.inputs.push(var(LIST_COUNT, "UDINT"));
            lowered.converts = true;
            continue;
        }
        // A scalar or a string is passed as it is, and so is a record or a
        // tuple of them passed as values, field after field.
        let single = shapes::single(resolve, &param.name, param.ty, here, scope);
        if in_memory && let Some(var) = &single {
            match kind(resolve, param.ty) {
                Some(kind) => lay(kind.bytes(), kind.bytes(), Some(offset)),
                // A string: its address, then its length.
                None => lay(8, 4, Some(offset)),
            }
            lowered.flat += var.flat;
        }
        let as_it_is = match single {
            Some(var) => Some(vec![(var.clone(), var.name)]),
            None if !in_memory => values(
                resolve,
                here,
                scope,
                &param.name,
                &param.name,
                param.ty,
                &At::Top,
            ),
            None => None,
        };
        if let Some(values) = as_it_is {
            let vars: Vec<Var> = values.iter().map(|(var, _)| var.clone()).collect();
            if !in_memory {
                lowered.flat += vars.iter().map(|var| var.flat).sum::<usize>();
            }
            // A value of a STRUCT is one input, each field of which the
            // import takes as it is.
            match types::structured(resolve, param.ty) {
                Some(id) => {
                    lowered.inputs.push(Var {
                        name: scope.variable(&param.name),
                        ty: shapes::type_name(resolve, id, here),
                        flat: 1,
                        start: None,
                    });
                    lowered
                        .sources
                        .extend(values.into_iter().map(|(var, source)| (var.name, source)));
                }
                None => lowered.inputs.extend(vars.iter().cloned()),
            }
            lowered.params.extend(vars);
            continue;
        }

        let mut walk = Walk {
            resolve,
            sizes,
            here,
            scope,
            inputs: Vec::new(),
        };
        let (steps, count) = walk
            .value(param.ty, offset, 0, &param.name, false, &At::Top)
            .map_err(|what| {
                format!(
                    "its parameter `{}` holds {what}, which an import cannot take yet",
                    param.name
                )
            })?;
        lowered.flat += count;
        let first = lowered.slots.len();
        let mut slot = |at: usize, bytes: usize, float: bool| {
            let name = format!("a{}", lowered.slots.len());
            lowered.slots.push(Slot {
                name,
                at,
                bytes,
                float,
            });
        };
        if in_memory {
            // The parameter's bytes, in words as wide as their place
            // allows: each is aligned as it is wide, so rk lays it out
            // where it is.
            let end = offset + sizes.size(&param.ty).size_wasm32();
            let mut at = offset;
            while at < end {
                let bytes = [8, 4, 2, 1]
                    .into_iter()
                    .find(|bytes| at.is_multiple_of(*bytes) && end - at >= *bytes)
                    .expect("a byte fits anywhere");
                lay(bytes, bytes, Some(at));
                slot(at, bytes, false);
                at += bytes;
            }
        } else {
            // Each value the parameter is passed as, as wide as the widest
            // any case puts there.
            let mut storage = [WasmType::I32; 64];
            let mut flat = FlatTypes::new(&mut storage);
            let fits = resolve.push_flat(&param.ty, &mut flat);
            let flat = flat.to_vec();
            assert!(
                fits && flat.len() == count,
                "a parameter is as many values as the canonical ABI flattens it into"
            );
            for (position, ty) in flat.into_iter().enumerate() {
                match ty {
                    WasmType::I32 => slot(position, 4, false),
                    WasmType::I64 => slot(position, 8, false),
                    WasmType::F32 => slot(position, 4, true),
                    WasmType::F64 => slot(position, 8, true),
                    WasmType::Pointer | WasmType::Length | WasmType::PointerOrI64 => {
                        unreachable!("a string in what is packed is refused")
                    }
                }
            }
        }
        let slots = &lowered.slots[first..];
        let mut converts = false;
        let mut statement = |put: Put| {
            let (slot, within) = match in_memory {
                true => {
                    let slot = slots
                        .iter()
                        .find(|slot| slot.at <= put.offset && put.offset < slot.at + slot.bytes)
                        .expect("a value lies in its parameter's bytes");
                    (slot, put.offset - slot.at)
                }
                false => (&slots[put.position], 0),
            };
            // A REAL or an LREAL slot takes its value as it is.
            converts |= put.kind.converts() && !slot.float;
            slot.put(within, put.kind, &put.source)
        };
        let steps: Vec<Step<String>> = steps
            .into_iter()
            .map(|step| step.map(&mut statement))
            .collect();
        lowered.steps.extend(steps);
        lowered.converts |= converts;
        lowered
            .params
            .extend(lowered.slots[first..].iter().map(Slot::var));
        lowered.inputs.extend(walk.inputs);
    }
    // rk passes an async call's parameters in memory past 4 values, as the
    // canonical ABI does: words that hold several values each are fewer,
    // and more of them make up the count.
    if in_memory && lowered.packs() {
        let mut passed: usize = lowered.params.iter().map(|var| var.flat).sum();
        while passed <= MAX_FLAT_ASYNC_PARAMS {
            let name = format!("a{}", lowered.slots.len());
            lowered.slots.push(Slot {
                name,
                at: 0,
                bytes: 8,
                float: false,
            });
            lowered
                .params
                .push(lowered.slots.last().expect("the word just made").var());
            passed += 1;
        }
    }
    Ok(lowered)
}

/// The values `ty` is passed as when the canonical ABI passes each as it
/// is, each with where a call takes it from: a scalar or a string is one,
/// the import's variable `name`; a record or a tuple is its fields' or its
/// items', named after it (`what_text`), taken from the members of the
/// STRUCT that holds them, the value named after `prefix` `at` the top or in
/// a STRUCT. `None` for what holds other values, which are packed.
fn values(
    resolve: &Resolve,
    here: Option<TypeOwner>,
    scope: &Scope,
    name: &str,
    prefix: &str,
    ty: Type,
    at: &At,
) -> Option<Vec<(Var, String)>> {
    if let Some(var) = shapes::single(resolve, name, ty, here, scope) {
        let source = match at {
            At::Top => var.name.clone(),
            At::Member(path) => At::member(path, prefix),
        };
        return Some(vec![(var, source)]);
    }
    if let Some(id) = types::structured(resolve, ty) {
        // A type with no STRUCT is refused where it is packed.
        types::declared(resolve, id, here).ok()?;
        let path = match at {
            At::Top => scope.variable(name),
            At::Member(path) => At::member(path, prefix),
        };
        let fields: Vec<(String, Type)> = match &resolve.types[types::target(resolve, id)].kind {
            TypeDefKind::Record(record) => record
                .fields
                .iter()
                .map(|field| (field.name.clone(), field.ty))
                .collect(),
            TypeDefKind::Tuple(tuple) => tuple
                .types
                .iter()
                .enumerate()
                .map(|(index, ty)| (format!("item{index}"), *ty))
                .collect(),
            _ => return None,
        };
        let mut values_of = Vec::new();
        for (field, ty) in fields {
            values_of.extend(values(
                resolve,
                here,
                scope,
                &join(name, &field),
                &field,
                ty,
                &At::Member(path.clone()),
            )?);
        }
        return Some(values_of);
    }
    match shapes::definition(resolve, ty) {
        Some(TypeDefKind::Tuple(tuple)) => {
            let mut values_of = Vec::new();
            for (index, item) in tuple.types.iter().enumerate() {
                let part = index.to_string();
                values_of.extend(values(
                    resolve,
                    here,
                    scope,
                    &join(name, &part),
                    &join(prefix, &part),
                    *item,
                    at,
                )?);
            }
            Some(values_of)
        }
        _ => None,
    }
}

/// A parameter's type walked value by value, its inputs as they come.
struct Walk<'a> {
    resolve: &'a Resolve,
    sizes: &'a SizeAlign,
    here: Option<TypeOwner>,
    scope: &'a Scope,
    inputs: Vec<Var>,
}

impl Walk<'_> {
    /// An input named after `name`, of the ST type `ty`, held as `kind`:
    /// its name in ST. `inside` a flag or a case, a call may leave it out,
    /// and it starts at zero.
    fn input(&mut self, name: &str, ty: String, kind: Kind, inside: bool) -> String {
        let name = self.scope.variable(name);
        self.inputs.push(Var {
            name: name.clone(),
            ty,
            flat: 1,
            start: inside.then(|| kind.zero()),
        });
        name
    }

    /// Where the value named after `prefix`, of the ST type `ty` and held
    /// as `kind`, is taken from: an input of its own at the top, a member
    /// of the STRUCT the walk is in otherwise.
    fn source(&mut self, prefix: &str, ty: String, kind: Kind, inside: bool, at: &At) -> String {
        match at {
            At::Top => self.input(prefix, ty, kind, inside),
            At::Member(path) => At::member(path, prefix),
        }
    }

    /// The steps that pack `ty`, which the canonical ABI has `offset` bytes
    /// into the parameters in memory and `position` values after its
    /// parameter's first, and how many values it is passed as. Its values
    /// are named after `prefix` `at` the top or in a STRUCT, as a result's
    /// are: a scalar is `prefix` itself; a value ST has a STRUCT for is one
    /// input of it, or one member, its own values in its members
    /// ([`Walk::members`]); a tuple's item adds its place; an option adds
    /// `is-some`, and `value` for an option it holds; a result adds
    /// `failed`, then `value` and `error`. `inside` a flag or a case, a
    /// value is only passed when that is.
    fn value(
        &mut self,
        ty: Type,
        offset: usize,
        position: usize,
        prefix: &str,
        inside: bool,
        at: &At,
    ) -> Result<(Vec<Step<Put>>, usize), &'static str> {
        let (resolve, sizes) = (self.resolve, self.sizes);
        if let Some(kind) = kind(resolve, ty) {
            let st = shapes::scalar(resolve, ty, self.here)
                .expect("a value packed into a word has a scalar type");
            let source = self.source(prefix, st, kind, inside, at);
            let put = Put {
                source,
                offset,
                position,
                kind,
            };
            return Ok((vec![Step::Leaf(put)], 1));
        }
        // A value ST has a STRUCT for is passed whole: one input, or one
        // member of the STRUCT it is in.
        if let Some(id) = types::structured(resolve, ty) {
            let path = match at {
                At::Member(path) => At::member(path, prefix),
                At::Top => {
                    let name = self.scope.variable(prefix);
                    self.inputs.push(Var {
                        name: name.clone(),
                        ty: shapes::type_name(resolve, id, self.here),
                        flat: 1,
                        start: None,
                    });
                    name
                }
            };
            // What the walk cannot pack says so first: a list, say.
            let packed = self.members(id, offset, position, inside, &path)?;
            types::declared(resolve, id, self.here)?;
            return Ok(packed);
        }
        let flag = |walk: &mut Self, name: &str| {
            let source = walk.source(name, "BOOL".to_string(), Kind::Bool, inside, at);
            let put = Put {
                source: source.clone(),
                offset,
                position,
                kind: Kind::Bool,
            };
            (source, Step::Leaf(put))
        };
        let Type::Id(id) = shapes::resolved(resolve, ty) else {
            return Err(shapes::what(resolve, ty));
        };
        match &resolve.types[id].kind {
            TypeDefKind::Tuple(tuple) => {
                let (mut steps, mut count) = (Vec::new(), 0);
                let offsets = sizes.field_offsets(&tuple.types);
                for (index, (place, item)) in offsets.into_iter().enumerate() {
                    let (more, values) = self.value(
                        *item,
                        offset + place.size_wasm32(),
                        position + count,
                        &join(prefix, &index.to_string()),
                        inside,
                        at,
                    )?;
                    steps.extend(more);
                    count += values;
                }
                Ok((steps, count))
            }
            TypeDefKind::Option(some) => {
                let held = offset
                    + sizes
                        .payload_offset(Int::U8, [None, Some(some)])
                        .size_wasm32();
                let (name, put) = flag(self, &join(prefix, "is-some"));
                let (then, values) = self.value(
                    *some,
                    held,
                    position + 1,
                    &held_by_option(resolve, prefix, *some),
                    true,
                    at,
                )?;
                let either = Step::Either {
                    flag: name,
                    then,
                    otherwise: Vec::new(),
                };
                Ok((vec![put, either], 1 + values))
            }
            TypeDefKind::Result(result) => {
                let held = offset
                    + sizes
                        .payload_offset(Int::U8, [result.ok.as_ref(), result.err.as_ref()])
                        .size_wasm32();
                let (name, put) = flag(self, &join(prefix, "failed"));
                let side = |walk: &mut Self, ty: Option<Type>, part: &str| match ty {
                    Some(ty) => walk.value(ty, held, position + 1, &join(prefix, part), true, at),
                    None => Ok((Vec::new(), 0)),
                };
                let (ok, ok_values) = side(self, result.ok, "value")?;
                let (error, error_values) = side(self, result.err, "error")?;
                let either = Step::Either {
                    flag: name,
                    then: error,
                    otherwise: ok,
                };
                Ok((vec![put, either], 1 + ok_values.max(error_values)))
            }
            _ => Err(shapes::what(resolve, ty)),
        }
    }

    /// The steps that pack the members of the STRUCT of `id`, the value at
    /// `path`, which the canonical ABI has `offset` bytes into the
    /// parameters in memory and `position` values after its parameter's
    /// first, and how many values it is passed as: a record's fields, a
    /// tuple's items, `item0`, `item1`, or a variant's case, `kind`, then
    /// what each case holds under the case's name.
    fn members(
        &mut self,
        id: TypeId,
        offset: usize,
        position: usize,
        inside: bool,
        path: &str,
    ) -> Result<(Vec<Step<Put>>, usize), &'static str> {
        let (resolve, sizes) = (self.resolve, self.sizes);
        let id = types::target(resolve, id);
        let at = At::Member(path.to_string());
        let (mut steps, mut count) = (Vec::new(), 0);
        match &resolve.types[id].kind {
            TypeDefKind::Record(record) => {
                let fields: Vec<Type> = record.fields.iter().map(|field| field.ty).collect();
                for (field, (place, _)) in record.fields.iter().zip(sizes.field_offsets(&fields)) {
                    let (more, values) = self.value(
                        field.ty,
                        offset + place.size_wasm32(),
                        position + count,
                        &field.name,
                        inside,
                        &at,
                    )?;
                    steps.extend(more);
                    count += values;
                }
                Ok((steps, count))
            }
            TypeDefKind::Tuple(tuple) => {
                let offsets = sizes.field_offsets(&tuple.types);
                for (index, (place, item)) in offsets.into_iter().enumerate() {
                    let (more, values) = self.value(
                        *item,
                        offset + place.size_wasm32(),
                        position + count,
                        &format!("item{index}"),
                        inside,
                        &at,
                    )?;
                    steps.extend(more);
                    count += values;
                }
                Ok((steps, count))
            }
            TypeDefKind::Variant(variant) => {
                let st = types::kind_name(resolve, id, self.here);
                let kind = tag_kind(variant.tag());
                let held = offset
                    + sizes
                        .payload_offset(variant.tag(), variant.cases.iter().map(|c| c.ty.as_ref()))
                        .size_wasm32();
                let selector = At::member(path, "kind");
                let put = Put {
                    source: selector.clone(),
                    offset,
                    position,
                    kind,
                };
                let (mut arms, mut widest) = (Vec::new(), 0);
                for case in &variant.cases {
                    let Some(ty) = case.ty else { continue };
                    let (more, values) =
                        self.value(ty, held, position + 1, &case.name, true, &at)?;
                    widest = widest.max(values);
                    if !more.is_empty() {
                        arms.push((format!("{st}#{}", names::upper(&case.name)), more));
                    }
                }
                let cases = Step::Cases { selector, arms };
                Ok((vec![Step::Leaf(put), cases], 1 + widest))
            }
            _ => Err(shapes::what(resolve, Type::Id(id))),
        }
    }
}
