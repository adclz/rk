use hir::hir_def::interned::identifier::Ident;

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
}

impl Default for MirMemoryLayout {
    fn default() -> Self {
        Self {
            offset: BUILTIN_RESERVED_FLOOR,
            allocations: Vec::new(),
        }
    }
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
