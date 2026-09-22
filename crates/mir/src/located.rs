//! Decoding a directly represented address (`%IX0.0`) into the band entry it
//! names.
//!
//! One decoder for both paths. A declaration carries its address as a HIR
//! node; an address written bare in a body reaches MIR as the text the place
//! was lowered under. The two have to agree about which cell an address is,
//! so they read it the same way — through here.

use hir::hir_def::pous::variable::LocationArea;

use crate::types::MirElementary;

/// What an address names, once decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressShape {
    pub area: LocationArea,
    /// Rank of the width letter: `X` < `B` < `W` < `D` < `L`.
    pub width_rank: u8,
    /// The numeric levels, in order. rk gives them no meaning of their own:
    /// they identify a cell and order it inside its band, nothing more.
    pub offsets: Vec<u32>,
}

impl AddressShape {
    /// The width letter's own type, which is what a bare address reads as.
    pub fn elementary(&self) -> MirElementary {
        match self.width_rank {
            0 => MirElementary::Bool,
            1 => MirElementary::Byte,
            2 => MirElementary::Word,
            3 => MirElementary::DWord,
            _ => MirElementary::LWord,
        }
    }

    /// What the width letter names, in bits.
    pub fn bits(&self) -> u16 {
        match self.width_rank {
            0 => 1,
            1 => 8,
            2 => 16,
            3 => 32,
            _ => 64,
        }
    }
}

/// The shape of an address AS WRITTEN (`%MW1.7.9`), or `None` when it names
/// no band the compiler has: an unknown area letter (`%Z0`), an unknown or
/// missing width letter (`%I1`), no level at all (`%IX`), or the incomplete
/// `%I*`, whose binding would come from VAR_CONFIG. `rk check` refuses each
/// of those with E1417 before lowering ever runs; this only has to agree
/// with it and never invent a band.
pub fn address_shape(text: &str) -> Option<AddressShape> {
    let rest = text.strip_prefix('%')?;
    let letters = rest
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect::<String>();
    let mut chars = letters.chars();
    let area = match chars.next()?.to_ascii_uppercase() {
        'I' => LocationArea::Input,
        'Q' => LocationArea::Output,
        'M' => LocationArea::Marker,
        _ => return None,
    };
    let width_rank = match chars.next()?.to_ascii_uppercase() {
        'X' => 0,
        'B' => 1,
        'W' => 2,
        'D' => 3,
        'L' => 4,
        _ => return None,
    };
    // Exactly the area letter and the width letter; `%IXQ0` is neither.
    if chars.next().is_some() {
        return None;
    }
    let mut offsets = Vec::new();
    for part in rest[letters.len()..].split('.') {
        // `unsigned_int` admits digit separators (`%IW1_000`); a level too
        // large to fit saturates, which only orders the cell last, and the
        // address TEXT stays what a host binds to.
        let digits = part.replace('_', "");
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        offsets.push(digits.parse::<u32>().unwrap_or(u32::MAX));
    }
    Some(AddressShape {
        area,
        width_rank,
        offsets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_decodes_to_its_area_width_and_levels() {
        let shape = address_shape("%MW1.7.9").expect("three levels");
        assert_eq!(shape.area, LocationArea::Marker);
        assert_eq!(shape.offsets, vec![1, 7, 9]);
        assert_eq!(shape.bits(), 16);
        assert_eq!(shape.elementary(), MirElementary::Word);

        assert_eq!(address_shape("%IX0.0").unwrap().offsets, vec![0, 0]);
        assert_eq!(address_shape("%qb7").unwrap().area, LocationArea::Output);
        assert_eq!(address_shape("%IW1_000").unwrap().offsets, vec![1000]);
    }

    /// Every shape `rk check` refuses (E1417) decodes to nothing here, so
    /// lowering cannot invent a band the front end said was not there.
    #[test]
    fn an_address_with_no_band_decodes_to_nothing() {
        for text in [
            "%I1",     // no width letter: Table 16 row 4b, not implemented
            "%Z0",     // no such area
            "%IZ0",    // no such width
            "%I*",     // incomplete; VAR_CONFIG would supply the binding
            "%IX",     // no level
            "%IX0.",   // empty level
            "%IXQ0.1", // more letters than area and width
            "IX0.0",   // no leading %
        ] {
            assert_eq!(address_shape(text), None, "{text} must name no band");
        }
    }
}
