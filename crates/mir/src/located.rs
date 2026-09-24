//! The type a directly represented address reads as.
//!
//! An address itself is decoded once, by HIR ([`LocatedAddress`]): its area,
//! its width and its levels. MIR takes that decoding and never reads the
//! text again, so the two cannot disagree about which cell an address is.
//!
//! [`LocatedAddress`]: hir::hir_def::pous::variable::LocatedAddress

use crate::types::MirElementary;

/// The bit-string type a width in bits names: what a bare address, or the
/// slice a view is, reads as.
pub fn width_elementary(bits: u8) -> MirElementary {
    match bits {
        1 => MirElementary::Bool,
        8 => MirElementary::Byte,
        16 => MirElementary::Word,
        32 => MirElementary::DWord,
        _ => MirElementary::LWord,
    }
}
