// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A result the generated code takes apart itself, as wit-bindgen's does.
//! The canonical ABI lays a variant out as its case, then one place the
//! values of all its cases share: no record of outputs is laid out so, nor
//! as a record of records, each record aligned on its own. A result that
//! holds one, a result with a value and an error among them, comes back
//! whole in words of 8 bytes, the bytes as the host wrote them, and the
//! code written here reads each value where WIT's layout puts it, into an
//! output or into its member of a STRUCT: a flag or a case first, then
//! what the case that came holds. What the other cases would hold stays at
//! zero. A record of scalars is read by no code: the import gives its
//! values as outputs, which go to their members ([`Lifted::as_outputs`]).
//!
//! A case whose value ST cannot hold yet, a list, keeps its name and comes
//! back without the value.
//!
//! A string is no value read out of the words: the host writes it where
//! the guest says, and a call that waits for its result says the STRING its
//! caller passed ([`Lifted::places`]). Nor is a list: the host writes its
//! elements into the caller's arrays, one per field of an element, an
//! `ARRAY[*]` of any length, and the words give how many came. One string
//! or one list comes back so, outside the result's error and outside a
//! variant's cases. A string inside them is a text, an error's or a case's,
//! which comes back into its member or its output, cut to fit, when it is
//! the one block the result can hold.

use wit_parser::{Int, Resolve, SizeAlign, Type, TypeDefKind, TypeId, TypeOwner};

use crate::Out;
use crate::names::{self, Scope};
use crate::shapes::{self, Var};
use crate::types;

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

/// Where the walk reads a value into, or packs one from: a variable of its
/// own, at the top of a result or of the parameters, or a member of a value
/// ST has a STRUCT for, by its path (`value.ipv4`).
#[derive(Debug, Clone)]
pub(crate) enum At {
    Top,
    Member(String),
}

impl At {
    /// The place of the value named after `prefix` in a STRUCT: its member.
    pub(crate) fn member(path: &str, prefix: &str) -> String {
        format!("{path}.{}", names::lower(prefix))
    }
}

/// What a value named after `prefix` is called where nothing names it:
/// `value`.
pub(crate) fn or_value(prefix: &str) -> String {
    match prefix {
        "" => "value".to_string(),
        prefix => prefix.to_string(),
    }
}

/// Where the host writes a block of the result: a STRING, or an array of
/// a list. A variable of the import, `var`, which a call passes `source`
/// for: a member of a STRUCT the call gives, or else the call's own
/// variable of that name. A text, an error's or a case's, is `cut` to fit
/// its STRING, the import's STRING `VAR_OUTPUT`.
#[derive(Debug, Clone)]
pub struct Place {
    pub var: Var,
    pub source: Option<String>,
    pub cut: bool,
}

/// A result taken apart: its outputs, and the code that gives them their
/// values.
#[derive(Debug)]
pub struct Lifted {
    /// The outputs, in the order the type gives its values: a flag or a
    /// case before what it decides.
    pub outputs: Vec<Var>,
    /// The type of the FUNCTION's own value, when the result is a value of
    /// a STRUCT the FUNCTION returns whole.
    pub returns: Option<String>,
    /// How many words of 8 bytes the result takes.
    words: usize,
    /// The values that are only there when their flag or their case came,
    /// each with what it holds otherwise.
    zeroes: Vec<(String, &'static str)>,
    steps: Vec<Step<(String, Read)>>,
    /// Where the host writes what the result holds in blocks of its own:
    /// the STRING of its string, the caller's, empty when no string came,
    /// the arrays of its list, one per field of an element, or the STRING
    /// of its text, cut to fit.
    pub places: Vec<Place>,
    /// Whether a value of a STRUCT comes back.
    pub structured: bool,
}

/// `ty`, a function's result, taken apart, its outputs named in `scope`;
/// `Err` says what in it cannot come back. A string can where `strings`
/// says so: in the result of a call that waits for it. A value of a STRUCT
/// that is the whole result is the FUNCTION's own value, `whole`, where it
/// has one.
pub fn lift(
    resolve: &Resolve,
    sizes: &SizeAlign,
    ty: Type,
    here: Option<TypeOwner>,
    scope: &Scope,
    strings: bool,
    whole: Option<&str>,
) -> Result<Lifted, &'static str> {
    let mut walk = Walk {
        resolve,
        sizes,
        here,
        scope,
        outputs: Vec::new(),
        zeroes: Vec::new(),
        strings,
        // A text comes back when it is the one block the result holds: a
        // call says where its one block goes, before it knows which side
        // or which case comes.
        texts: strings && blocks_in(resolve, ty) == 1,
        in_text: false,
        places: Vec::new(),
        taken: 0,
        whole: whole.map(str::to_string),
        returns: None,
        structured: false,
    };
    let steps = walk.value(ty, 0, "", false, &At::Top)?;
    // The host writes the blocks it is given, in order, into the STRING or
    // the arrays: any other block it could write, a string or a list where
    // nothing takes it, would land there first.
    if walk.taken > 0 && blocks_in(resolve, ty) != walk.taken {
        return Err("two strings or a string and a list");
    }
    Ok(Lifted {
        outputs: walk.outputs,
        returns: walk.returns,
        words: sizes.size(&ty).size_wasm32().div_ceil(8).max(1),
        zeroes: walk.zeroes,
        steps,
        places: walk.places,
        structured: walk.structured,
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
/// something cannot: the case then comes back as its name alone. With
/// `texts`, as a call that waits for it reads it, one text of it can.
pub fn unreadable(
    resolve: &Resolve,
    sizes: &SizeAlign,
    ty: Type,
    here: Option<TypeOwner>,
    texts: bool,
) -> Option<&'static str> {
    Walk {
        resolve,
        sizes,
        here,
        scope: &Scope::default(),
        outputs: Vec::new(),
        zeroes: Vec::new(),
        strings: false,
        texts,
        in_text: true,
        places: Vec::new(),
        taken: 0,
        whole: None,
        returns: None,
        structured: false,
    }
    .value(ty, 0, "case", true, &At::Top)
    .err()
}

/// The ST type of the one scalar `ty` holds when it is one value: a
/// scalar, or a record or a tuple of one, which the canonical ABI returns
/// as the value it holds.
fn sole_scalar(resolve: &Resolve, ty: Type, here: Option<TypeOwner>) -> Option<String> {
    if kind(resolve, ty).is_some() {
        return shapes::scalar(resolve, ty, here);
    }
    match shapes::definition(resolve, ty)? {
        TypeDefKind::Record(record) if record.fields.len() == 1 => {
            sole_scalar(resolve, record.fields[0].ty, here)
        }
        TypeDefKind::Tuple(tuple) if tuple.types.len() == 1 => {
            sole_scalar(resolve, tuple.types[0], here)
        }
        _ => None,
    }
}

/// The ST types of the values of `ty`, a value of a STRUCT, when the
/// canonical ABI lays it out as a record of scalars, which an import
/// gives as its outputs: a record or a tuple of scalars, or one value
/// inside records or tuples that hold it alone. A record of records is
/// not one: each record inside is aligned on its own, and rounded up.
pub fn record_of_scalars(
    resolve: &Resolve,
    ty: Type,
    here: Option<TypeOwner>,
) -> Option<Vec<String>> {
    let id = types::structured(resolve, ty)?;
    if let Some(st) = sole_scalar(resolve, ty, here) {
        return Some(vec![st]);
    }
    let parts: Vec<Type> = match &resolve.types[types::target(resolve, id)].kind {
        TypeDefKind::Record(record) => record.fields.iter().map(|field| field.ty).collect(),
        TypeDefKind::Tuple(tuple) => tuple.types.clone(),
        _ => return None,
    };
    parts
        .into_iter()
        .map(|ty| match kind(resolve, ty) {
            Some(_) => shapes::scalar(resolve, ty, here),
            None => None,
        })
        .collect()
}

impl Lifted {
    /// The outputs an import gives a record of scalars as, the ST types of
    /// its values `scalars` ([`record_of_scalars`]), each with the member
    /// the generated code puts it in. Each is named after its member,
    /// `seconds`, or `inner_x` for one inside a record of one, with a `_`
    /// after it for as long as `held`, the names beside it, or an earlier
    /// one has the name.
    pub fn as_outputs(&self, scalars: Vec<String>, mut held: Vec<String>) -> Vec<(Var, String)> {
        let targets: Vec<&str> = self
            .steps
            .iter()
            .map(|step| match step {
                Step::Leaf((target, _)) => target.as_str(),
                _ => unreachable!("a record of scalars is read value by value"),
            })
            .collect();
        assert_eq!(targets.len(), scalars.len(), "a member for each value");
        targets
            .into_iter()
            .zip(scalars)
            .map(|(target, st)| {
                let (_, path) = target
                    .split_once('.')
                    .expect("a member of the value that comes back");
                let mut name = names::lower(&path.replace('.', "_"));
                while held.iter().any(|held| held.eq_ignore_ascii_case(&name)) {
                    name.push('_');
                }
                held.push(name.clone());
                let output = Var {
                    name,
                    ty: st,
                    flat: 1,
                    start: None,
                };
                (output, target.to_string())
            })
            .collect()
    }

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

/// The arrays the elements of a list of `element` go into, by name, each
/// with its elements' ST type: one for a scalar or a string, named as the
/// list is (`value` where nothing names it), and one per field of a record
/// or a tuple of them, named as their outputs would be. A byte is a BYTE.
/// `None` for an element that holds more.
fn list_arrays(
    resolve: &Resolve,
    here: Option<TypeOwner>,
    element: Type,
    prefix: &str,
) -> Option<Vec<(String, String)>> {
    let one = |ty: Type| match shapes::resolved(resolve, ty) {
        Type::U8 => Some("BYTE".to_string()),
        Type::String => Some("STRING".to_string()),
        _ => shapes::scalar(resolve, ty, here),
    };
    if let Some(st) = one(element) {
        let name = match prefix {
            "" => "value".to_string(),
            prefix => prefix.to_string(),
        };
        return Some(vec![(name, st)]);
    }
    let fields: Vec<(String, Type)> = match shapes::definition(resolve, element)? {
        TypeDefKind::Record(record) => record
            .fields
            .iter()
            .map(|field| (join(prefix, &field.name), field.ty))
            .collect(),
        TypeDefKind::Tuple(tuple) => tuple
            .types
            .iter()
            .enumerate()
            .map(|(at, ty)| match prefix {
                "" => (format!("item{at}"), *ty),
                prefix => (join(prefix, &at.to_string()), *ty),
            })
            .collect(),
        _ => return None,
    };
    fields
        .into_iter()
        .map(|(name, ty)| one(ty).map(|st| (name, st)))
        .collect()
}

/// A result's type walked value by value, its outputs as they come.
struct Walk<'a> {
    resolve: &'a Resolve,
    sizes: &'a SizeAlign,
    here: Option<TypeOwner>,
    scope: &'a Scope,
    outputs: Vec<Var>,
    zeroes: Vec<(String, &'static str)>,
    /// Whether a string or a list may come back outside an error and a
    /// case.
    strings: bool,
    /// Whether a text may come back: a string inside an error or a case.
    texts: bool,
    /// Whether the walk is inside an error or a case.
    in_text: bool,
    /// The STRING the one string that comes back is written into, or the
    /// arrays of the one list.
    places: Vec<Place>,
    /// How many blocks those take, as [`blocks_in`] counts them.
    taken: usize,
    /// The FUNCTION whose own value a STRUCT that is the whole result is.
    whole: Option<String>,
    /// The type of that value, once the walk has met it.
    returns: Option<String>,
    /// Whether a value of a STRUCT comes back.
    structured: bool,
}

impl Walk<'_> {
    /// What `ty` holds, walked as an error or as what a variant's case
    /// holds: a string in it is a text, and no list comes back.
    fn as_text<T>(&mut self, walk: impl FnOnce(&mut Self) -> T) -> T {
        let in_text = std::mem::replace(&mut self.in_text, true);
        let walked = walk(self);
        self.in_text = in_text;
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

    /// Where the value named after `prefix`, of the ST type `ty` and read
    /// as `kind`, goes: an output of its own at the top, a member of the
    /// STRUCT the walk is in otherwise. `inside` a flag or a case, it holds
    /// zero when that did not come.
    fn target(&mut self, prefix: &str, ty: String, kind: Kind, inside: bool, at: &At) -> String {
        match at {
            At::Top => self.output(&or_value(prefix), ty, kind, inside),
            At::Member(path) => {
                let member = At::member(path, prefix);
                if inside {
                    self.zeroes.push((member.clone(), kind.zero()));
                }
                member
            }
        }
    }

    /// The steps that read `ty` at `offset` bytes into the result, its
    /// values named after `prefix` `at` the top or in a STRUCT: a scalar is
    /// `prefix` itself, `value` where there is none; a value ST has a STRUCT
    /// for is one output of it, or one member, its own values in its
    /// members ([`Walk::members`]); a tuple's item adds its place; an option
    /// adds `is-some`, and `value` for an option it holds; a result adds
    /// `failed`, then `value` and `error`. `inside` a flag or a case, a
    /// value is only there when that came.
    fn value(
        &mut self,
        ty: Type,
        offset: usize,
        prefix: &str,
        inside: bool,
        at: &At,
    ) -> Result<Vec<Step<(String, Read)>>, &'static str> {
        let (resolve, sizes) = (self.resolve, self.sizes);
        if let Some(kind) = kind(resolve, ty) {
            let st = shapes::scalar(resolve, ty, self.here)
                .expect("a value read out of a word has a scalar type");
            let target = self.target(prefix, st, kind, inside, at);
            return Ok(vec![Step::Leaf((target, Read { offset, kind }))]);
        }
        // A string is not read: the host wrote it into the STRING the call
        // passed, which is empty when none came, cut to fit for a text.
        if let Type::String = shapes::resolved(resolve, ty) {
            let comes = match self.in_text {
                true => self.texts,
                false => self.strings,
            };
            return match (comes, self.taken) {
                (false, _) => Err("a string"),
                (true, 0) => {
                    let (name, source) = match at {
                        At::Top => (or_value(prefix), None),
                        At::Member(path) => (prefix.to_string(), Some(At::member(path, prefix))),
                    };
                    // A text's STRING is as long as a member's: one the
                    // FUNCTION gives as an output, where no member holds it.
                    let ty = match self.in_text {
                        true => types::member_string(),
                        false => "STRING".to_string(),
                    };
                    self.places.push(Place {
                        var: Var {
                            name: self.scope.variable(&name),
                            ty,
                            flat: 0,
                            start: None,
                        },
                        source,
                        cut: self.in_text,
                    });
                    self.taken = 1;
                    Ok(Vec::new())
                }
                (true, _) => Err("two strings or a string and a list"),
            };
        }
        // A value ST has a STRUCT for comes back whole: the FUNCTION's own
        // value when it is the result, an output or a member otherwise.
        if let Some(id) = types::structured(resolve, ty) {
            self.structured = true;
            let path = match at {
                At::Member(path) => At::member(path, prefix),
                At::Top => {
                    let st = shapes::type_name(resolve, id, self.here);
                    match (&self.whole, prefix) {
                        (Some(pou), "") => {
                            self.returns = Some(st);
                            pou.clone()
                        }
                        _ => {
                            let name = self.scope.variable(&or_value(prefix));
                            self.outputs.push(Var {
                                name: name.clone(),
                                ty: st,
                                flat: 1,
                                start: None,
                            });
                            name
                        }
                    }
                }
            };
            // What the walk cannot read says so first: a list, say.
            let steps = self.members(id, offset, inside, &path)?;
            types::declared(resolve, id, self.here)?;
            return Ok(steps);
        }
        let flag = |walk: &mut Self, name: &str| {
            let target = walk.target(name, "BOOL".to_string(), Kind::Bool, inside, at);
            let read = Read {
                offset,
                kind: Kind::Bool,
            };
            (target.clone(), Step::Leaf((target, read)))
        };
        let Type::Id(id) = shapes::resolved(resolve, ty) else {
            return Err(shapes::what(resolve, ty));
        };
        match &resolve.types[id].kind {
            // A list: its elements in the caller's arrays, one per field,
            // and how many came, its length, which the words hold. A STRUCT
            // holds no list.
            TypeDefKind::List(element) => {
                let arrays = match (self.strings && !self.in_text, at) {
                    (true, At::Top) => {
                        list_arrays(resolve, self.here, *element, prefix).ok_or("a list")?
                    }
                    _ => return Err("a list"),
                };
                for (name, element) in arrays {
                    self.places.push(Place {
                        var: Var {
                            name: self.scope.variable(&name),
                            ty: format!("ARRAY[*] OF {element}"),
                            flat: 0,
                            start: None,
                        },
                        source: None,
                        cut: false,
                    });
                }
                self.taken = 2;
                let count = match prefix {
                    "" => "count".to_string(),
                    prefix => join(prefix, "count"),
                };
                let kind = Kind::Unsigned(4);
                let count = self.output(&count, "UDINT".to_string(), kind, inside);
                let read = Read {
                    offset: offset + 4,
                    kind,
                };
                Ok(vec![Step::Leaf((count, read))])
            }
            TypeDefKind::Tuple(tuple) => {
                let mut steps = Vec::new();
                let offsets = sizes.field_offsets(&tuple.types);
                for (index, (place, item)) in offsets.into_iter().enumerate() {
                    let part = match (prefix, at) {
                        ("", At::Top) => format!("item{index}"),
                        _ => index.to_string(),
                    };
                    steps.extend(self.value(
                        *item,
                        offset + place.size_wasm32(),
                        &join(prefix, &part),
                        inside,
                        at,
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
                let then = self.value(
                    *some,
                    held,
                    &held_by_option(resolve, prefix, *some),
                    true,
                    at,
                )?;
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
                    Some(ty) => walk.value(ty, held, &join(prefix, part), true, at),
                    None => Ok(Vec::new()),
                };
                let ok = side(self, result.ok, "value")?;
                let error = self.as_text(|walk| side(walk, result.err, "error"))?;
                Ok(vec![
                    set,
                    Step::Either {
                        flag: name,
                        then: error,
                        otherwise: ok,
                    },
                ])
            }
            _ => Err(shapes::what(resolve, ty)),
        }
    }

    /// The steps that read the members of the STRUCT of `id`, at `offset`
    /// bytes into the result, into the value at `path`: a record's fields,
    /// a tuple's items, `item0`, `item1`, or a variant's case, `kind`, then
    /// what each case holds under the case's name. A case whose value ST
    /// cannot hold yet keeps its name alone: the file says so where the
    /// variant is.
    fn members(
        &mut self,
        id: TypeId,
        offset: usize,
        inside: bool,
        path: &str,
    ) -> Result<Vec<Step<(String, Read)>>, &'static str> {
        let (resolve, sizes) = (self.resolve, self.sizes);
        let id = types::target(resolve, id);
        let at = At::Member(path.to_string());
        let mut steps = Vec::new();
        match &resolve.types[id].kind {
            TypeDefKind::Record(record) => {
                let fields: Vec<Type> = record.fields.iter().map(|field| field.ty).collect();
                for (field, (place, _)) in record.fields.iter().zip(sizes.field_offsets(&fields)) {
                    steps.extend(self.value(
                        field.ty,
                        offset + place.size_wasm32(),
                        &field.name,
                        inside,
                        &at,
                    )?);
                }
                Ok(steps)
            }
            TypeDefKind::Tuple(tuple) => {
                let offsets = sizes.field_offsets(&tuple.types);
                for (index, (place, item)) in offsets.into_iter().enumerate() {
                    steps.extend(self.value(
                        *item,
                        offset + place.size_wasm32(),
                        &format!("item{index}"),
                        inside,
                        &at,
                    )?);
                }
                Ok(steps)
            }
            TypeDefKind::Variant(variant) => {
                let st = types::kind_name(resolve, id, self.here);
                let kind = tag_kind(variant.tag());
                let held = offset
                    + sizes
                        .payload_offset(variant.tag(), variant.cases.iter().map(|c| c.ty.as_ref()))
                        .size_wasm32();
                let selector = At::member(path, "kind");
                if inside {
                    self.zeroes.push((selector.clone(), kind.zero()));
                }
                let mut arms = Vec::new();
                for case in &variant.cases {
                    let Some(ty) = case.ty else { continue };
                    let (outputs, zeroes) = (self.outputs.len(), self.zeroes.len());
                    let (places, taken) = (self.places.len(), self.taken);
                    let walked = self.as_text(|walk| walk.value(ty, held, &case.name, true, &at));
                    match walked {
                        Ok(steps) if steps.is_empty() => {}
                        Ok(steps) => {
                            arms.push((format!("{st}#{}", names::upper(&case.name)), steps));
                        }
                        Err(_) => {
                            self.outputs.truncate(outputs);
                            self.zeroes.truncate(zeroes);
                            self.places.truncate(places);
                            self.taken = taken;
                        }
                    }
                }
                Ok(vec![
                    Step::Leaf((selector.clone(), Read { offset, kind })),
                    Step::Cases { selector, arms },
                ])
            }
            _ => Err(shapes::what(resolve, Type::Id(id))),
        }
    }
}
