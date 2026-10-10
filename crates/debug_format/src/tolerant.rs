//! Positional MessagePack, read so that a newer producer's record still
//! decodes.
//!
//! `rmp_serde` writes a struct as an array of its fields, in order, and
//! refuses an array longer than the struct it is read into. A field added at
//! the tail by a newer compiler therefore cost an older runtime the whole
//! section, and the runtime on a device is usually the older of the two.
//!
//! Reading through [`from_slice`] takes the fields this build knows and
//! skips what follows them, at every depth: a record inside a list inside a
//! section, and the fields of an enum's variant. The bytes are the ones
//! `rmp_serde::to_vec` writes; only the reader changed.
//!
//! What it does not absorb: a field removed or reordered, and an enum
//! variant this build has never heard of. Those still need a version bump.

use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, IgnoredAny, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};
use std::fmt;

/// Decode `bytes` as `rmp_serde::from_slice` does, skipping the trailing
/// fields of any record that has more than `T` declares.
pub(crate) fn from_slice<'de, T: de::Deserialize<'de>>(
    bytes: &'de [u8],
) -> Result<T, rmp_serde::decode::Error> {
    let mut de = rmp_serde::Deserializer::from_read_ref(bytes);
    T::deserialize(Tolerant(&mut de))
}

/// A deserializer whose every sequence forgives a longer input. It wraps
/// each visitor and each nested deserializer, so the forgiveness reaches the
/// records inside.
struct Tolerant<D>(D);

/// The same wrapper on the visiting side: a visitor, a seed, or one of the
/// accessors serde hands a visitor.
struct Wrap<T>(T);

macro_rules! forward_to_inner {
    ($($method:ident)*) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
            self.0.$method(Wrap(visitor))
        }
    )*};
}

impl<'de, D: Deserializer<'de>> Deserializer<'de> for Tolerant<D> {
    type Error = D::Error;

    forward_to_inner! {
        deserialize_any deserialize_bool
        deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64 deserialize_i128
        deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64 deserialize_u128
        deserialize_f32 deserialize_f64 deserialize_char
        deserialize_str deserialize_string deserialize_bytes deserialize_byte_buf
        deserialize_option deserialize_unit deserialize_seq deserialize_map
        deserialize_identifier deserialize_ignored_any
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.0.deserialize_unit_struct(name, Wrap(visitor))
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.0.deserialize_newtype_struct(name, Wrap(visitor))
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.0.deserialize_tuple(len, Wrap(visitor))
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.0.deserialize_tuple_struct(name, len, Wrap(visitor))
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.0.deserialize_struct(name, fields, Wrap(visitor))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.0.deserialize_enum(name, variants, Wrap(visitor))
    }

    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

macro_rules! forward_values {
    ($($method:ident($ty:ty))*) => {$(
        fn $method<E: de::Error>(self, v: $ty) -> Result<V::Value, E> {
            self.0.$method(v)
        }
    )*};
}

impl<'de, V: Visitor<'de>> Visitor<'de> for Wrap<V> {
    type Value = V::Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.expecting(f)
    }

    forward_values! {
        visit_bool(bool)
        visit_i8(i8) visit_i16(i16) visit_i32(i32) visit_i64(i64) visit_i128(i128)
        visit_u8(u8) visit_u16(u16) visit_u32(u32) visit_u64(u64) visit_u128(u128)
        visit_f32(f32) visit_f64(f64) visit_char(char)
        visit_str(&str) visit_borrowed_str(&'de str) visit_string(String)
        visit_bytes(&[u8]) visit_borrowed_bytes(&'de [u8]) visit_byte_buf(Vec<u8>)
    }

    fn visit_none<E: de::Error>(self) -> Result<V::Value, E> {
        self.0.visit_none()
    }

    fn visit_unit<E: de::Error>(self) -> Result<V::Value, E> {
        self.0.visit_unit()
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<V::Value, D::Error> {
        self.0.visit_some(Tolerant(d))
    }

    fn visit_newtype_struct<D: Deserializer<'de>>(self, d: D) -> Result<V::Value, D::Error> {
        self.0.visit_newtype_struct(Tolerant(d))
    }

    /// The one place that differs from a plain read: what the visitor left
    /// in the sequence is a newer producer's fields, read and dropped.
    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<V::Value, A::Error> {
        let mut seq = Wrap(seq);
        let value = self.0.visit_seq(&mut seq)?;
        while seq.0.next_element::<IgnoredAny>()?.is_some() {}
        Ok(value)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<V::Value, A::Error> {
        self.0.visit_map(Wrap(map))
    }

    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<V::Value, A::Error> {
        self.0.visit_enum(Wrap(data))
    }
}

impl<'de, S: DeserializeSeed<'de>> DeserializeSeed<'de> for Wrap<S> {
    type Value = S::Value;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<S::Value, D::Error> {
        self.0.deserialize(Tolerant(d))
    }
}

impl<'de, A: SeqAccess<'de>> SeqAccess<'de> for Wrap<A> {
    type Error = A::Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, A::Error> {
        self.0.next_element_seed(Wrap(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for Wrap<A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, A::Error> {
        self.0.next_key_seed(Wrap(seed))
    }

    fn next_value_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<T::Value, A::Error> {
        self.0.next_value_seed(Wrap(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}

impl<'de, A: EnumAccess<'de>> EnumAccess<'de> for Wrap<A> {
    type Error = A::Error;
    type Variant = Wrap<A::Variant>;

    fn variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<(T::Value, Self::Variant), A::Error> {
        let (value, variant) = self.0.variant_seed(Wrap(seed))?;
        Ok((value, Wrap(variant)))
    }
}

impl<'de, A: VariantAccess<'de>> VariantAccess<'de> for Wrap<A> {
    type Error = A::Error;

    fn unit_variant(self) -> Result<(), A::Error> {
        self.0.unit_variant()
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, A::Error> {
        self.0.newtype_variant_seed(Wrap(seed))
    }

    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, A::Error> {
        self.0.tuple_variant(len, Wrap(visitor))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, A::Error> {
        self.0.struct_variant(fields, Wrap(visitor))
    }
}

#[cfg(test)]
mod tests {
    use super::from_slice;
    use serde::{Deserialize, Serialize};

    /// What this build knows.
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Leaf {
        name: String,
        address: u32,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Shape {
        Scalar(u8),
        Record { size: u32, leaves: Vec<Leaf> },
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Table {
        version: u16,
        leaves: Vec<Leaf>,
        shapes: Vec<Shape>,
        #[serde(default)]
        note: Option<String>,
    }

    /// The same records as a newer producer writes them: a field at the tail
    /// of each, at every depth.
    #[derive(Serialize)]
    struct NewerLeaf {
        name: String,
        address: u32,
        unit: String,
        scale: (f64, f64),
    }

    #[derive(Serialize)]
    enum NewerShape {
        #[allow(dead_code)]
        Scalar(u8),
        Record {
            size: u32,
            leaves: Vec<NewerLeaf>,
            align: u32,
        },
    }

    #[derive(Serialize)]
    struct NewerTable {
        version: u16,
        leaves: Vec<NewerLeaf>,
        shapes: Vec<NewerShape>,
        note: Option<String>,
        checksum: Vec<u8>,
    }

    fn newer_leaf(name: &str, address: u32) -> NewerLeaf {
        NewerLeaf {
            name: name.to_string(),
            address,
            unit: "rpm".to_string(),
            scale: (0.0, 1.5),
        }
    }

    /// A newer producer's fields are skipped in the section, in each record
    /// of a list and inside an enum's variant, and what this build knows
    /// comes out whole.
    #[test]
    fn trailing_fields_are_skipped_at_every_depth() {
        let newer = NewerTable {
            version: 9,
            leaves: vec![newer_leaf("P1.speed", 1024), newer_leaf("P1.run", 1028)],
            shapes: vec![NewerShape::Record {
                size: 8,
                leaves: vec![newer_leaf("x", 0)],
                align: 4,
            }],
            note: Some("kept".to_string()),
            checksum: vec![1, 2, 3],
        };
        let bytes = rmp_serde::to_vec(&newer).unwrap();

        assert!(
            rmp_serde::from_slice::<Table>(&bytes).is_err(),
            "the plain reader refuses the longer records: that was the fault"
        );
        let table: Table = from_slice(&bytes).unwrap();
        assert_eq!(
            table,
            Table {
                version: 9,
                leaves: vec![
                    Leaf {
                        name: "P1.speed".to_string(),
                        address: 1024
                    },
                    Leaf {
                        name: "P1.run".to_string(),
                        address: 1028
                    },
                ],
                shapes: vec![Shape::Record {
                    size: 8,
                    leaves: vec![Leaf {
                        name: "x".to_string(),
                        address: 0
                    }],
                }],
                note: Some("kept".to_string()),
            }
        );
    }

    /// An older producer's shorter record still decodes: a missing tail
    /// field takes its default, as before.
    #[test]
    fn a_shorter_record_takes_its_defaults() {
        #[derive(Serialize)]
        struct OlderTable {
            version: u16,
            leaves: Vec<Leaf>,
            shapes: Vec<Shape>,
        }
        let bytes = rmp_serde::to_vec(&OlderTable {
            version: 1,
            leaves: Vec::new(),
            shapes: vec![Shape::Scalar(3)],
        })
        .unwrap();
        let table: Table = from_slice(&bytes).unwrap();
        assert_eq!(table.note, None);
        assert_eq!(table.shapes, vec![Shape::Scalar(3)]);
    }

    /// What tolerance does not cover stays an error: a field of the wrong
    /// type, and a record too short for a field with no default.
    #[test]
    fn a_wrong_or_missing_field_is_still_refused() {
        let wrong = rmp_serde::to_vec(&("not a version", 0u8, 0u8)).unwrap();
        assert!(from_slice::<Table>(&wrong).is_err());
        let short = rmp_serde::to_vec(&(1u16,)).unwrap();
        assert!(from_slice::<Table>(&short).is_err());
    }

    /// A variant is written by its name, not its index: the comments that
    /// kept a variant last "for the encoding" were guarding nothing, and a
    /// variant can be added anywhere without moving the others.
    #[test]
    fn an_enum_variant_is_written_by_name() {
        let unit = rmp_serde::to_vec(&crate::SymType::LReal).unwrap();
        assert_eq!(&unit[1..], b"LReal", "a fixstr of the name");
        let data = rmp_serde::to_vec(&Shape::Scalar(7)).unwrap();
        assert!(
            data.windows(6).any(|w| w == b"Scalar"),
            "a one-entry map keyed by the name: {data:?}"
        );
    }

    /// A value this build writes reads back the same through both readers.
    #[test]
    fn a_current_record_reads_as_the_plain_reader_reads_it() {
        let table = Table {
            version: 1,
            leaves: vec![Leaf {
                name: "a".to_string(),
                address: 4,
            }],
            shapes: vec![Shape::Scalar(1)],
            note: None,
        };
        let bytes = rmp_serde::to_vec(&table).unwrap();
        assert_eq!(from_slice::<Table>(&bytes).unwrap(), table);
        assert_eq!(rmp_serde::from_slice::<Table>(&bytes).unwrap(), table);
    }
}
