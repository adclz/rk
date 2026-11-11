// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! How the component model's canonical ABI lays out the values rk passes
//! across a component's boundary: each scalar's WIT type, size and
//! alignment, a STRING's `(ptr, len)`, and a record of them, which is what
//! a FUNCTION with `VAR_OUTPUT`s returns. The WIT `rk compile` writes and
//! the code that fills or reads a record both come from here.

use mir::types::{MirElementary, MirType};
use wasm_encoder::Instruction;

use crate::emit_expr::mem_arg;

/// A value as the canonical ABI holds it in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Canonical {
    /// Its WIT type.
    pub wit: &'static str,
    pub size: u32,
    pub align: u32,
    /// What rk holds it as, `None` for a STRING.
    pub elementary: Option<MirElementary>,
}

/// `ty` as the canonical ABI holds it: a scalar, an enum as its storage and
/// a subrange as its base, or a STRING. `None` for anything else.
pub fn canonical(ty: &MirType) -> Option<Canonical> {
    use MirElementary::*;
    let elementary = match ty {
        MirType::String { .. } => {
            return Some(Canonical {
                wit: "string",
                size: 8,
                align: 4,
                elementary: None,
            });
        }
        MirType::Elementary(elementary) => *elementary,
        MirType::Enum(enumeration) => enumeration.storage,
        MirType::Subrange(subrange) => subrange.base,
        _ => return None,
    };
    let (wit, size) = match elementary {
        Bool => ("bool", 1),
        SInt => ("s8", 1),
        USInt | Byte => ("u8", 1),
        Int => ("s16", 2),
        UInt | Word => ("u16", 2),
        DInt | Time | Date | Tod => ("s32", 4),
        UDInt | DWord => ("u32", 4),
        Real => ("f32", 4),
        Char => ("char", 4),
        LInt | LTime | LDate | LTod | DateAndTime | LDateTime => ("s64", 8),
        ULInt | LWord => ("u64", 8),
        LReal => ("f64", 8),
    };
    Some(Canonical {
        wit,
        size,
        align: size,
        elementary: Some(elementary),
    })
}

/// Where each field of a record is, and the record's size and alignment:
/// each field at the next multiple of its alignment, the record aligned as
/// its most aligned field and padded to a multiple of it.
pub fn record_layout(fields: &[Canonical]) -> (Vec<u32>, u32, u32) {
    let mut offsets = Vec::with_capacity(fields.len());
    let mut end = 0u32;
    let mut align = 1;
    for field in fields {
        let at = end.next_multiple_of(field.align);
        offsets.push(at);
        end = at + field.size;
        align = align.max(field.align);
    }
    (offsets, end.next_multiple_of(align), align)
}

/// Where the payload of a `result<T, string>` is, past its tag: at the
/// first multiple of the larger of `T`'s alignment and a string's.
pub fn result_payload(align: u32) -> u64 {
    u64::from(align.max(4))
}

/// The load of a scalar of `elementary` from where the canonical ABI holds
/// it, `offset` bytes past the address on the stack, onto rk's lane: an
/// 8-bit or 16-bit integer extended by its sign.
pub fn load(elementary: MirElementary, offset: u64) -> Instruction<'static> {
    use MirElementary::*;
    match elementary {
        Bool | USInt | Byte => Instruction::I32Load8U(mem_arg(offset, 0)),
        SInt => Instruction::I32Load8S(mem_arg(offset, 0)),
        Int => Instruction::I32Load16S(mem_arg(offset, 1)),
        UInt | Word => Instruction::I32Load16U(mem_arg(offset, 1)),
        Real => Instruction::F32Load(mem_arg(offset, 2)),
        LReal => Instruction::F64Load(mem_arg(offset, 3)),
        wide if wide.is_64bit() => Instruction::I64Load(mem_arg(offset, 3)),
        _ => Instruction::I32Load(mem_arg(offset, 2)),
    }
}

/// The store of a scalar of `elementary` where the canonical ABI holds it,
/// `offset` bytes past the address on the stack: a BOOL and an 8-bit
/// integer in one byte, a 16-bit one in two.
pub fn store(elementary: MirElementary, offset: u64) -> Instruction<'static> {
    use MirElementary::*;
    match elementary {
        Bool | SInt | USInt | Byte => Instruction::I32Store8(mem_arg(offset, 0)),
        Int | UInt | Word => Instruction::I32Store16(mem_arg(offset, 1)),
        Real => Instruction::F32Store(mem_arg(offset, 2)),
        LReal => Instruction::F64Store(mem_arg(offset, 3)),
        wide if wide.is_64bit() => Instruction::I64Store(mem_arg(offset, 3)),
        _ => Instruction::I32Store(mem_arg(offset, 2)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(elementary: MirElementary) -> Canonical {
        canonical(&MirType::Elementary(elementary)).unwrap()
    }

    /// Each field at the next multiple of its own alignment, the record as
    /// aligned as its most aligned field: `{ bool, s64, s16 }` is the bool at
    /// 0, the s64 at 8, the s16 at 16, and 24 bytes.
    #[test]
    fn a_record_aligns_each_field() {
        let fields = [
            scalar(MirElementary::Bool),
            scalar(MirElementary::LInt),
            scalar(MirElementary::Int),
        ];
        assert_eq!(record_layout(&fields), (vec![0, 8, 16], 24, 8));
        let small = [scalar(MirElementary::Bool), scalar(MirElementary::Byte)];
        assert_eq!(record_layout(&small), (vec![0, 1], 2, 1));
        assert_eq!(result_payload(1), 4);
        assert_eq!(result_payload(8), 8);
    }
}
