use hir::hir_def::interned::identifier::Ident;
use rustc_hash::FxHashMap;

/// Bytes reserved at the bottom of linear memory before any IEC allocation:
/// the grafted `wasm_builtins` use `[0, 8192)` as their shadow stack and
/// `[8192, 9008)` for their data segments; rounded up to 16 KiB.
pub const BUILTIN_RESERVED_FLOOR: u32 = 16_384;

/// Memory layout for the entire module - fully resolved during MIR lowering.
#[derive(Debug, Clone)]
pub struct MirMemoryLayout {
    /// Current offset (next available address).
    offset: u32,
    /// All allocations in order.
    pub allocations: Vec<MirAllocation>,
    /// Addresses of `Retain` variables as allocated, before band relocation.
    pub retain_allocations: Vec<RetainEntry>,
}

impl Default for MirMemoryLayout {
    fn default() -> Self {
        Self {
            offset: BUILTIN_RESERVED_FLOOR,
            allocations: Vec::new(),
            retain_allocations: Vec::new(),
        }
    }
}

/// A variable that must persist across power cycles (declared `RETAIN`).
/// `address` is its current linear-memory address (pre-band-relocation).
#[derive(Debug, Clone, Copy)]
pub struct RetainEntry {
    pub name: Ident,
    pub address: u32,
    pub size: u32,
    pub align: u32,
}

/// Result of relocating all RETAIN variables into one contiguous band.
#[derive(Debug, Clone)]
pub struct RetainBand {
    /// Start address of the band.
    pub base: u32,
    /// Total byte length of the band (0 when there are no retained vars).
    pub size: u32,
    /// Old address → new (in-band) address. The caller must apply this to
    /// every `MirLocal.storage` so reads/writes hit the relocated bytes.
    pub remap: FxHashMap<u32, u32>,
}

/// A single allocation in linear memory.
#[derive(Debug, Clone)]
pub struct MirAllocation {
    pub name: Ident,
    pub address: u32,
    pub size: u32,
    pub align: u32,
    pub kind: MirAllocKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirAllocKind {
    /// A variable (local, instance field, etc.).
    Variable,
    /// String literal data.
    StringData,
    /// Instance data (FB/Class).
    InstanceData,
}

impl MirMemoryLayout {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate `size` bytes at `align`; returns the address.
    pub fn allocate(&mut self, name: Ident, size: u32, align: u32, kind: MirAllocKind) -> u32 {
        // Align current offset
        let address = align_to(self.offset, align);
        self.allocations.push(MirAllocation {
            name,
            address,
            size,
            align,
            kind,
        });
        self.offset = address + size;
        address
    }

    /// Register an allocated variable as RETAIN for the band; pure
    /// bookkeeping.
    pub fn record_retain(&mut self, name: Ident, address: u32, size: u32, align: u32) {
        self.retain_allocations.push(RetainEntry {
            name,
            address,
            size,
            align,
        });
    }

    /// Relocate every RETAIN variable into one contiguous band at the top of
    /// the arena; returns the bounds and the old→new remap. The original
    /// slots stay as holes.
    pub fn finalize_retain_band(&mut self) -> RetainBand {
        if self.retain_allocations.is_empty() {
            return RetainBand {
                base: self.offset,
                size: 0,
                remap: FxHashMap::default(),
            };
        }
        // Align the band base to the largest alignment it contains, so the
        // band starts on a clean boundary and every entry inside stays aligned.
        let band_align = self
            .retain_allocations
            .iter()
            .map(|r| r.align)
            .max()
            .unwrap_or(1);
        let base = align_to(self.offset, band_align);
        let mut cursor = base;
        let mut remap = FxHashMap::default();
        for entry in &self.retain_allocations {
            let addr = align_to(cursor, entry.align);
            remap.insert(entry.address, addr);
            cursor = addr + entry.size;
        }
        self.offset = cursor;
        RetainBand {
            base,
            size: cursor - base,
            remap,
        }
    }

    /// Total memory size used (useful for WASM memory section).
    pub fn total_size(&self) -> u32 {
        self.offset
    }
}

/// Align `offset` up to the next multiple of `align`.
pub fn align_to(offset: u32, align: u32) -> u32 {
    if align == 0 {
        return offset;
    }
    (offset + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_to() {
        assert_eq!(align_to(0, 4), 0);
        assert_eq!(align_to(1, 4), 4);
        assert_eq!(align_to(4, 4), 4);
        assert_eq!(align_to(5, 4), 8);
        assert_eq!(align_to(7, 8), 8);
        assert_eq!(align_to(8, 8), 8);
    }
}
