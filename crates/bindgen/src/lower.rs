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
//! The inputs a call gives are the values themselves, named as a result's
//! outputs are. One that is only passed with its case starts at zero, so a
//! call names the case it passes and no other.

use wit_parser::abi::{FlatTypes, WasmType};
use wit_parser::{Function, Int, Resolve, SizeAlign, Type, TypeDefKind, TypeOwner};

use crate::lift::{self, Kind, Step, held_by_option, join, kind, tag_kind};
use crate::names::{self, Scope};
use crate::shapes::{self, Var};
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

/// A function's parameters, as a call gives them and as its import takes
/// them.
#[derive(Debug, Default)]
pub struct Lowered {
    /// The inputs a call gives: each value by its name. One that is only
    /// passed with its case starts at zero.
    pub inputs: Vec<Var>,
    /// The import's parameters, in order: an input passed as it is, or a
    /// word values are packed into.
    pub params: Vec<Var>,
    /// How many values the canonical ABI flattens the parameters into.
    pub flat: usize,
    slots: Vec<Slot>,
    steps: Vec<Step<String>>,
}

impl Lowered {
    /// Whether values are packed into words: the import's parameters are
    /// then not the inputs a call gives.
    pub fn packs(&self) -> bool {
        !self.slots.is_empty()
    }

    /// The words values are packed into.
    pub fn words(&self) -> Vec<Var> {
        self.slots.iter().map(Slot::var).collect()
    }

    /// Writes the statements that pack the inputs into the words: each
    /// word at zero first, then each value that is passed.
    pub(crate) fn write(&self, out: &mut Out) {
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
            Some(var) => Some(vec![var]),
            None if !in_memory => shapes::input(resolve, &param.name, param.ty, here, scope).ok(),
            None => None,
        };
        if let Some(vars) = as_it_is {
            if !in_memory {
                lowered.flat += vars.iter().map(|var| var.flat).sum::<usize>();
            }
            lowered.inputs.extend(vars.iter().cloned());
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
            .value(param.ty, offset, 0, &param.name, false)
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
        let mut statement = |put: Put| match in_memory {
            true => {
                let slot = slots
                    .iter()
                    .find(|slot| slot.at <= put.offset && put.offset < slot.at + slot.bytes)
                    .expect("a value lies in its parameter's bytes");
                slot.put(put.offset - slot.at, put.kind, &put.source)
            }
            false => slots[put.position].put(0, put.kind, &put.source),
        };
        let steps: Vec<Step<String>> = steps
            .into_iter()
            .map(|step| step.map(&mut statement))
            .collect();
        lowered.steps.extend(steps);
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

    /// The steps that pack `ty`, which the canonical ABI has `offset` bytes
    /// into the parameters in memory and `position` values after its
    /// parameter's first, and how many values it is passed as. Its inputs
    /// are named after `prefix`, as a result's outputs are: a scalar is
    /// `prefix` itself; a record's field and a tuple's item add their name
    /// or their place; an option adds `is-some`, and `value` for an option
    /// it holds; a result adds `failed`, then `value` and `error`; a
    /// variant is its case, then what each case holds under the case's
    /// name. `inside` a flag or a case, a value is only passed when that
    /// is.
    fn value(
        &mut self,
        ty: Type,
        offset: usize,
        position: usize,
        prefix: &str,
        inside: bool,
    ) -> Result<(Vec<Step<Put>>, usize), &'static str> {
        let (resolve, sizes) = (self.resolve, self.sizes);
        if let Some(kind) = kind(resolve, ty) {
            let st = shapes::scalar(resolve, ty, self.here)
                .expect("a value packed into a word has a scalar type");
            let source = self.input(prefix, st, kind, inside);
            let put = Put {
                source,
                offset,
                position,
                kind,
            };
            return Ok((vec![Step::Leaf(put)], 1));
        }
        let flag = |walk: &mut Self, name: &str| {
            let source = walk.input(name, "BOOL".to_string(), Kind::Bool, inside);
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
            TypeDefKind::Record(record) => {
                let types: Vec<Type> = record.fields.iter().map(|field| field.ty).collect();
                let (mut steps, mut count) = (Vec::new(), 0);
                for (field, (at, _)) in record.fields.iter().zip(sizes.field_offsets(&types)) {
                    let (more, values) = self.value(
                        field.ty,
                        offset + at.size_wasm32(),
                        position + count,
                        &join(prefix, &field.name),
                        inside,
                    )?;
                    steps.extend(more);
                    count += values;
                }
                Ok((steps, count))
            }
            TypeDefKind::Tuple(tuple) => {
                let (mut steps, mut count) = (Vec::new(), 0);
                let offsets = sizes.field_offsets(&tuple.types);
                for (index, (at, item)) in offsets.into_iter().enumerate() {
                    let (more, values) = self.value(
                        *item,
                        offset + at.size_wasm32(),
                        position + count,
                        &join(prefix, &index.to_string()),
                        inside,
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
                    Some(ty) => walk.value(ty, held, position + 1, &join(prefix, part), true),
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
            TypeDefKind::Variant(variant) => {
                let st = shapes::type_name(resolve, id, self.here);
                let kind = tag_kind(variant.tag());
                let held = offset
                    + sizes
                        .payload_offset(variant.tag(), variant.cases.iter().map(|c| c.ty.as_ref()))
                        .size_wasm32();
                let selector = self.input(prefix, st.clone(), kind, inside);
                let put = Put {
                    source: selector.clone(),
                    offset,
                    position,
                    kind,
                };
                let (mut arms, mut widest) = (Vec::new(), 0);
                for case in &variant.cases {
                    let Some(ty) = case.ty else { continue };
                    let (steps, values) =
                        self.value(ty, held, position + 1, &join(prefix, &case.name), true)?;
                    widest = widest.max(values);
                    if !steps.is_empty() {
                        arms.push((format!("{st}#{}", names::upper(&case.name)), steps));
                    }
                }
                let cases = Step::Cases { selector, arms };
                Ok((vec![Step::Leaf(put), cases], 1 + widest))
            }
            _ => Err(shapes::what(resolve, ty)),
        }
    }
}
