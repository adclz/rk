// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A result the generated code takes apart itself, as wit-bindgen's does.
//! The canonical ABI lays a variant out as its case, then one place the
//! values of all its cases share: no record of outputs is laid out so. A
//! result that holds one, a result with a value and an error among them,
//! comes back whole in words of 8 bytes, the bytes as the host wrote them,
//! and the code written here reads each value where WIT's layout puts it:
//! a flag or a case first, then what the case that came holds. What the
//! other cases would hold stays at zero.
//!
//! A case whose value ST cannot hold yet, a string or a list, keeps its
//! name and comes back without the value.
//!
//! A string is no value read out of the words: the host writes it where
//! the guest says, and a call that waits for its result says the STRING its
//! caller passed ([`Lifted::text`]). One string comes back so, the one a
//! result holds outside its error and outside a variant's cases.

use wit_parser::{Int, Resolve, SizeAlign, Type, TypeDefKind, TypeOwner};

use crate::Out;
use crate::names::{self, Scope};
use crate::shapes::{self, Var};

/// How a value is read out of a word or packed into one, by its size in
/// bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Bool,
    Unsigned(usize),
    Signed(usize),
    Float(usize),
    /// A bit string, taken as it is: flags.
    Bits(usize),
}

/// How a scalar `ty` is read: `None` for what [`shapes::scalar`] has no
/// type for.
pub(crate) fn kind(resolve: &Resolve, ty: Type) -> Option<Kind> {
    Some(match shapes::resolved(resolve, ty) {
        Type::Bool => Kind::Bool,
        Type::U8 => Kind::Unsigned(1),
        Type::U16 => Kind::Unsigned(2),
        Type::U32 => Kind::Unsigned(4),
        Type::U64 => Kind::Unsigned(8),
        Type::S8 => Kind::Signed(1),
        Type::S16 => Kind::Signed(2),
        Type::S32 => Kind::Signed(4),
        Type::S64 => Kind::Signed(8),
        Type::F32 => Kind::Float(4),
        Type::F64 => Kind::Float(8),
        Type::Char | Type::String | Type::ErrorContext => return None,
        Type::Id(id) => match &resolve.types[id].kind {
            TypeDefKind::Enum(enumeration) => tag_kind(enumeration.tag()),
            TypeDefKind::Variant(variant) if !shapes::carries(variant) => tag_kind(variant.tag()),
            TypeDefKind::Flags(flags) => Kind::Bits(shapes::flags_base(flags)?.1),
            TypeDefKind::Handle(_)
            | TypeDefKind::Resource
            | TypeDefKind::Future(_)
            | TypeDefKind::Stream(_) => Kind::Unsigned(4),
            _ => return None,
        },
    })
}

impl Kind {
    /// Whether the value is converted on its way into or out of a word,
    /// with `Std.Convert`: a flag and a bit string are taken as they are.
    pub(crate) fn converts(self) -> bool {
        !matches!(self, Kind::Bool | Kind::Bits(_))
    }

    /// What a value that did not come holds.
    pub(crate) fn zero(self) -> &'static str {
        match self {
            Kind::Bool => "FALSE",
            Kind::Float(_) => "0.0",
            _ => "0",
        }
    }

    /// How many bytes the canonical ABI holds the value in.
    pub(crate) fn bytes(self) -> usize {
        match self {
            Kind::Bool => 1,
            Kind::Unsigned(bytes)
            | Kind::Signed(bytes)
            | Kind::Float(bytes)
            | Kind::Bits(bytes) => bytes,
        }
    }
}

/// The unsigned integer a case is held in, as a value is read.
pub(crate) fn tag_kind(tag: Int) -> Kind {
    Kind::Unsigned(match tag {
        Int::U8 => 1,
        Int::U16 => 2,
        Int::U32 => 4,
        Int::U64 => 8,
    })
}

/// A value's place in the result, in bytes from its start, and how it is
/// read there.
#[derive(Debug, Clone, Copy)]
struct Read {
    offset: usize,
    kind: Kind,
}

impl Read {
    /// The expression that reads the value out of the words `word0`,
    /// `word1`, 8 bytes each: the part of a word the value is, as the type
    /// it is.
    fn expression(self, word: &str) -> String {
        let whole = format!("{word}{}", self.offset / 8);
        let within = self.offset % 8;
        let part = |bytes: usize| {
            assert_eq!(
                within % bytes,
                0,
                "the canonical ABI aligns a value to its size"
            );
            match bytes {
                1 => format!("{whole}.%B{within}"),
                2 => format!("{whole}.%W{}", within / 2),
                4 => format!("{whole}.%D{}", within / 4),
                _ => whole.clone(),
            }
        };
        let converted = |names: [&str; 4], bytes: usize| {
            format!(
                "{}({})",
                names[bytes.trailing_zeros() as usize],
                part(bytes)
            )
        };
        match self.kind {
            Kind::Bool => format!("{whole}.%X{}", within * 8),
            Kind::Bits(bytes) => part(bytes),
            Kind::Unsigned(bytes) => converted(
                [
                    "BYTE_TO_USINT",
                    "WORD_TO_UINT",
                    "DWORD_TO_UDINT",
                    "LWORD_TO_ULINT",
                ],
                bytes,
            ),
            Kind::Signed(bytes) => converted(
                [
                    "BYTE_TO_SINT",
                    "WORD_TO_INT",
                    "DWORD_TO_DINT",
                    "LWORD_TO_LINT",
                ],
                bytes,
            ),
            Kind::Float(bytes) => converted(["", "", "DWORD_TO_REAL", "LWORD_TO_LREAL"], bytes),
        }
    }
}

/// A statement of the code that reads a result or packs a parameter: what
/// is done with one value, or what a flag or a case decides.
#[derive(Debug)]
pub(crate) enum Step<L> {
    /// One value, read or packed.
    Leaf(L),
    /// What a flag decides: one side when it is TRUE, the other when not.
    Either {
        flag: String,
        then: Vec<Step<L>>,
        otherwise: Vec<Step<L>>,
    },
    /// What each case of a variant holds, by the case's name.
    Cases {
        selector: String,
        arms: Vec<(String, Vec<Step<L>>)>,
    },
}

impl<L> Step<L> {
    /// The step with each of its values turned into what `leaf` makes of
    /// it.
    pub(crate) fn map<M>(self, leaf: &mut dyn FnMut(L) -> M) -> Step<M> {
        let all = |steps: Vec<Step<L>>, leaf: &mut dyn FnMut(L) -> M| -> Vec<Step<M>> {
            steps.into_iter().map(|step| step.map(leaf)).collect()
        };
        match self {
            Step::Leaf(value) => Step::Leaf(leaf(value)),
            Step::Either {
                flag,
                then,
                otherwise,
            } => Step::Either {
                flag,
                then: all(then, leaf),
                otherwise: all(otherwise, leaf),
            },
            Step::Cases { selector, arms } => Step::Cases {
                selector,
                arms: arms
                    .into_iter()
                    .map(|(label, steps)| (label, all(steps, leaf)))
                    .collect(),
            },
        }
    }
}

/// A result taken apart: its outputs, and the code that gives them their
/// values.
#[derive(Debug)]
pub struct Lifted {
    /// The outputs, in the order the type gives its values: a flag or a
    /// case before what it decides.
    pub outputs: Vec<Var>,
    /// How many words of 8 bytes the result takes.
    words: usize,
    /// The outputs that are only there when their flag or their case came,
    /// each with what it holds otherwise.
    zeroes: Vec<(String, &'static str)>,
    steps: Vec<Step<(String, Read)>>,
    /// The STRING the result's string is written into, by the host: the
    /// caller's, empty when no string came.
    pub text: Option<Var>,
}

/// `ty`, a function's result, taken apart, its outputs named in `scope`;
/// `Err` says what in it cannot come back. A string can where `strings`
/// says so: in the result of a call that waits for it.
pub fn lift(
    resolve: &Resolve,
    sizes: &SizeAlign,
    ty: Type,
    here: Option<TypeOwner>,
    scope: &Scope,
    strings: bool,
) -> Result<Lifted, &'static str> {
    let mut walk = Walk {
        resolve,
        sizes,
        here,
        scope,
        outputs: Vec::new(),
        zeroes: Vec::new(),
        strings,
        text: None,
    };
    let steps = walk.value(ty, 0, "", false)?;
    // The host writes the first block it is given into the STRING: any
    // other block it could write, a string or a list where no output takes
    // it, would land there first.
    if walk.text.is_some() && blocks_in(resolve, ty) > 1 {
        return Err("two strings or a string and a list");
    }
    Ok(Lifted {
        outputs: walk.outputs,
        words: sizes.size(&ty).size_wasm32().div_ceil(8).max(1),
        zeroes: walk.zeroes,
        steps,
        text: walk.text,
    })
}

/// How many blocks the host may write for a value of `ty`, each at its own
/// address: a string's bytes, and for a list its elements and what they
/// hold, counted as two at least. Wherever they are: each field, each side
/// of a result, each case of a variant.
fn blocks_in(resolve: &Resolve, ty: Type) -> usize {
    let Type::Id(id) = shapes::resolved(resolve, ty) else {
        return usize::from(shapes::resolved(resolve, ty) == Type::String);
    };
    let sum = |types: &mut dyn Iterator<Item = Type>| types.map(|ty| blocks_in(resolve, ty)).sum();
    match &resolve.types[id].kind {
        TypeDefKind::Record(record) => sum(&mut record.fields.iter().map(|field| field.ty)),
        TypeDefKind::Tuple(tuple) => sum(&mut tuple.types.iter().copied()),
        TypeDefKind::Option(some) => blocks_in(resolve, *some),
        TypeDefKind::Result(result) => sum(&mut result.ok.into_iter().chain(result.err)),
        TypeDefKind::Variant(variant) => sum(&mut variant.cases.iter().filter_map(|case| case.ty)),
        TypeDefKind::List(_) | TypeDefKind::Map(..) => 2,
        TypeDefKind::FixedLengthList(element, _) => 2 * blocks_in(resolve, *element),
        _ => 0,
    }
}

/// What in `ty`, the value a variant's case holds, cannot come back, when
/// something cannot: the case then comes back as its name alone.
pub fn unreadable(
    resolve: &Resolve,
    sizes: &SizeAlign,
    ty: Type,
    here: Option<TypeOwner>,
) -> Option<&'static str> {
    Walk {
        resolve,
        sizes,
        here,
        scope: &Scope::default(),
        outputs: Vec::new(),
        zeroes: Vec::new(),
        strings: false,
        text: None,
    }
    .value(ty, 0, "case", true)
    .err()
}

impl Lifted {
    /// The words the result comes back in, `w0`, `w1`: `at_least` of them,
    /// where the call takes more than the result fills.
    pub fn words(&self, at_least: usize) -> Vec<Var> {
        (0..self.words.max(at_least))
            .map(|at| Var {
                name: format!("w{at}"),
                ty: "LWORD".to_string(),
                flat: 1,
                start: None,
            })
            .collect()
    }

    /// Whether the code that reads the result converts a value, and needs
    /// `Std.Convert`.
    pub fn converts(&self) -> bool {
        any(&self.steps, &|(_, read)| read.kind.converts())
    }

    /// Writes the statements that give each output its value, read from
    /// the words named `word` and their number.
    pub(crate) fn write(&self, out: &mut Out, word: &str) {
        for (name, zero) in &self.zeroes {
            out.line(&format!("{name} := {zero};"));
        }
        write(out, &self.steps, &|(name, read)| {
            format!("{name} := {};", read.expression(word))
        });
    }
}

/// Whether a value of `steps` is one `leaf` says yes to.
pub(crate) fn any<L>(steps: &[Step<L>], leaf: &dyn Fn(&L) -> bool) -> bool {
    steps.iter().any(|step| match step {
        Step::Leaf(value) => leaf(value),
        Step::Either {
            then, otherwise, ..
        } => any(then, leaf) || any(otherwise, leaf),
        Step::Cases { arms, .. } => arms.iter().any(|(_, steps)| any(steps, leaf)),
    })
}

/// Writes `steps`, each value's statement as `leaf` writes it.
pub(crate) fn write<L>(out: &mut Out, steps: &[Step<L>], leaf: &dyn Fn(&L) -> String) {
    let block = |out: &mut Out, steps: &[Step<L>]| {
        out.depth += 1;
        write(out, steps, leaf);
        out.depth -= 1;
    };
    for step in steps {
        match step {
            Step::Leaf(value) => out.line(&leaf(value)),
            Step::Either {
                flag,
                then,
                otherwise,
            } => match (then.is_empty(), otherwise.is_empty()) {
                (true, true) => {}
                (false, true) => {
                    out.line(&format!("IF {flag} THEN"));
                    block(out, then);
                    out.line("END_IF;");
                }
                (true, false) => {
                    out.line(&format!("IF NOT {flag} THEN"));
                    block(out, otherwise);
                    out.line("END_IF;");
                }
                (false, false) => {
                    out.line(&format!("IF {flag} THEN"));
                    block(out, then);
                    out.line("ELSE");
                    block(out, otherwise);
                    out.line("END_IF;");
                }
            },
            Step::Cases { selector, arms } if !arms.is_empty() => {
                out.line(&format!("CASE {selector} OF"));
                out.depth += 1;
                for (label, steps) in arms {
                    out.line(&format!("{label}:"));
                    block(out, steps);
                }
                out.depth -= 1;
                out.line("END_CASE;");
            }
            Step::Cases { .. } => {}
        }
    }
}

/// `part` after `prefix`, as WIT joins words.
pub(crate) fn join(prefix: &str, part: &str) -> String {
    match prefix {
        "" => part.to_string(),
        prefix => format!("{prefix}-{part}"),
    }
}

/// What the value an option holds, `some`, is named after, the option
/// named after `prefix`: the option's own name, `value` where it has none,
/// and `value` after it for an option, whose `is-some` would be the first's.
pub(crate) fn held_by_option(resolve: &Resolve, prefix: &str, some: Type) -> String {
    match (prefix, shapes::definition(resolve, some)) {
        ("", _) | (_, Some(TypeDefKind::Option(_))) => join(prefix, "value"),
        _ => prefix.to_string(),
    }
}

/// A result's type walked value by value, its outputs as they come.
struct Walk<'a> {
    resolve: &'a Resolve,
    sizes: &'a SizeAlign,
    here: Option<TypeOwner>,
    scope: &'a Scope,
    outputs: Vec<Var>,
    zeroes: Vec<(String, &'static str)>,
    /// Whether a string may come back where the walk is.
    strings: bool,
    /// The STRING the one string that comes back is written into.
    text: Option<Var>,
}

impl Walk<'_> {
    /// What `ty` holds, walked with no string allowed in it: an error, or
    /// what a variant's case holds.
    fn without_strings<T>(&mut self, walk: impl FnOnce(&mut Self) -> T) -> T {
        let strings = std::mem::replace(&mut self.strings, false);
        let walked = walk(self);
        self.strings = strings;
        walked
    }

    /// An output named after `name`, of the ST type `ty`, read as `kind`:
    /// its name in ST. `inside` a flag or a case, it holds zero when that
    /// did not come.
    fn output(&mut self, name: &str, ty: String, kind: Kind, inside: bool) -> String {
        let name = self.scope.variable(name);
        if inside {
            self.zeroes.push((name.clone(), kind.zero()));
        }
        self.outputs.push(Var {
            name: name.clone(),
            ty,
            flat: 1,
            start: None,
        });
        name
    }

    /// The steps that read `ty` at `offset` bytes into the result, its
    /// values named after `prefix`: a scalar is `prefix` itself, `value`
    /// where there is none; a record's field and a tuple's item add their
    /// name or their place; an option adds `is-some`, and `value` for an
    /// option it holds; a result adds `failed`, then `value` and `error`;
    /// a variant is its case, then what each case holds under the case's
    /// name. `inside` a flag or a case, a value is only there when that
    /// came.
    fn value(
        &mut self,
        ty: Type,
        offset: usize,
        prefix: &str,
        inside: bool,
    ) -> Result<Vec<Step<(String, Read)>>, &'static str> {
        let (resolve, sizes) = (self.resolve, self.sizes);
        let or_value = |prefix: &str| match prefix {
            "" => "value".to_string(),
            prefix => prefix.to_string(),
        };
        if let Some(kind) = kind(resolve, ty) {
            let st = shapes::scalar(resolve, ty, self.here)
                .expect("a value read out of a word has a scalar type");
            let name = self.output(&or_value(prefix), st, kind, inside);
            return Ok(vec![Step::Leaf((name, Read { offset, kind }))]);
        }
        let flag = |walk: &mut Self, name: &str| {
            let name = walk.output(name, "BOOL".to_string(), Kind::Bool, inside);
            let read = Read {
                offset,
                kind: Kind::Bool,
            };
            (name.clone(), Step::Leaf((name, read)))
        };
        // A string is not read: the host wrote it into the STRING the call
        // passed, which is empty when none came.
        if let Type::String = shapes::resolved(resolve, ty) {
            return match (self.strings, &self.text) {
                (false, _) => Err("a string"),
                (true, Some(_)) => Err("two strings"),
                (true, None) => {
                    self.text = Some(Var {
                        name: self.scope.variable(&or_value(prefix)),
                        ty: "STRING".to_string(),
                        flat: 0,
                        start: None,
                    });
                    Ok(Vec::new())
                }
            };
        }
        let Type::Id(id) = shapes::resolved(resolve, ty) else {
            return Err(shapes::what(resolve, ty));
        };
        match &resolve.types[id].kind {
            TypeDefKind::Record(record) => {
                let types: Vec<Type> = record.fields.iter().map(|field| field.ty).collect();
                let mut steps = Vec::new();
                for (field, (at, _)) in record.fields.iter().zip(sizes.field_offsets(&types)) {
                    steps.extend(self.value(
                        field.ty,
                        offset + at.size_wasm32(),
                        &join(prefix, &field.name),
                        inside,
                    )?);
                }
                Ok(steps)
            }
            TypeDefKind::Tuple(tuple) => {
                let mut steps = Vec::new();
                let offsets = sizes.field_offsets(&tuple.types);
                for (index, (at, item)) in offsets.into_iter().enumerate() {
                    let part = match prefix {
                        "" => format!("item{index}"),
                        _ => index.to_string(),
                    };
                    steps.extend(self.value(
                        *item,
                        offset + at.size_wasm32(),
                        &join(prefix, &part),
                        inside,
                    )?);
                }
                Ok(steps)
            }
            TypeDefKind::Option(some) => {
                let held = offset
                    + sizes
                        .payload_offset(Int::U8, [None, Some(some)])
                        .size_wasm32();
                let (name, set) = flag(self, &join(prefix, "is-some"));
                let then =
                    self.value(*some, held, &held_by_option(resolve, prefix, *some), true)?;
                Ok(vec![
                    set,
                    Step::Either {
                        flag: name,
                        then,
                        otherwise: Vec::new(),
                    },
                ])
            }
            TypeDefKind::Result(result) => {
                let held = offset
                    + sizes
                        .payload_offset(Int::U8, [result.ok.as_ref(), result.err.as_ref()])
                        .size_wasm32();
                let (name, set) = flag(self, &join(prefix, "failed"));
                let side = |walk: &mut Self, ty: Option<Type>, part: &str| match ty {
                    Some(ty) => walk.value(ty, held, &join(prefix, part), true),
                    None => Ok(Vec::new()),
                };
                let ok = side(self, result.ok, "value")?;
                let error = self.without_strings(|walk| side(walk, result.err, "error"))?;
                Ok(vec![
                    set,
                    Step::Either {
                        flag: name,
                        then: error,
                        otherwise: ok,
                    },
                ])
            }
            TypeDefKind::Variant(variant) => {
                let st = shapes::type_name(resolve, id, self.here);
                let kind = tag_kind(variant.tag());
                let held = offset
                    + sizes
                        .payload_offset(variant.tag(), variant.cases.iter().map(|c| c.ty.as_ref()))
                        .size_wasm32();
                let selector = self.output(&or_value(prefix), st.clone(), kind, inside);
                let mut arms = Vec::new();
                for case in &variant.cases {
                    let Some(ty) = case.ty else { continue };
                    // A case whose value cannot come back keeps its name
                    // alone: the file says so where the variant is.
                    let (outputs, zeroes) = (self.outputs.len(), self.zeroes.len());
                    let walked = self.without_strings(|walk| {
                        walk.value(ty, held, &join(prefix, &case.name), true)
                    });
                    match walked {
                        Ok(steps) if steps.is_empty() => {}
                        Ok(steps) => {
                            arms.push((format!("{st}#{}", names::upper(&case.name)), steps));
                        }
                        Err(_) => {
                            self.outputs.truncate(outputs);
                            self.zeroes.truncate(zeroes);
                        }
                    }
                }
                Ok(vec![
                    Step::Leaf((selector.clone(), Read { offset, kind })),
                    Step::Cases { selector, arms },
                ])
            }
            _ => Err(shapes::what(resolve, ty)),
        }
    }
}
