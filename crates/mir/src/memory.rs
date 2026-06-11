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
    /// Every config/resource VAR_GLOBAL, recorded as it is allocated, so they
    /// can be gathered into one contiguous, host-visible band. `retain` globals
    /// are also persisted: the band is laid out so they sit in the overlap of
    /// the globals band and the retain band.
    pub global_allocations: Vec<GlobalEntry>,
}

impl Default for MirMemoryLayout {
    fn default() -> Self {
        Self {
            offset: BUILTIN_RESERVED_FLOOR,
            allocations: Vec::new(),
            retain_allocations: Vec::new(),
            global_allocations: Vec::new(),
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

/// A config/resource VAR_GLOBAL, recorded as it is allocated. `retain` globals
/// are persisted and sit in the overlap of the globals and retain bands.
#[derive(Debug, Clone, Copy)]
pub struct GlobalEntry {
    pub name: Ident,
    pub address: u32,
    pub size: u32,
    pub align: u32,
    pub retain: bool,
}

/// Result of relocating globals + RETAIN variables into contiguous bands.
/// The globals band `[globals_base, globals_size)` holds every config/resource
/// VAR_GLOBAL (host-visible); the retain band `[retain_base, retain_size)` holds
/// the retained set (host-snapshotted). Retain globals are the overlap of the
/// two — laid out at the globals band's end / the retain band's start.
#[derive(Debug, Clone)]
pub struct MemoryBands {
    pub globals_base: u32,
    pub globals_size: u32,
    pub retain_base: u32,
    pub retain_size: u32,
    /// Old address → in-band address; applied to every stored absolute address.
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

    /// Register an already-allocated config/resource VAR_GLOBAL so it can be
    /// gathered into the contiguous host-visible band. Pure bookkeeping.
    pub fn record_global(&mut self, name: Ident, address: u32, size: u32, align: u32, retain: bool) {
        self.global_allocations.push(GlobalEntry {
            name,
            address,
            size,
            align,
            retain,
        });
    }

    /// Relocate every RETAIN variable into one contiguous band at the top of
    /// the arena; returns the bounds and the old→new remap. The original
    /// slots stay as holes.
    pub fn finalize_bands(&mut self) -> MemoryBands {
        let retain_fields = std::mem::take(&mut self.retain_allocations);
        let globals = std::mem::take(&mut self.global_allocations);
        if globals.is_empty() && retain_fields.is_empty() {
            let p = self.offset;
            return MemoryBands {
                globals_base: p,
                globals_size: 0,
                retain_base: p,
                retain_size: 0,
                remap: FxHashMap::default(),
            };
        }
        // `[ non-retain globals | retain globals | retain program-instances ]`,
        // contiguous at the arena top.
        let (retain_globals, nonretain_globals): (Vec<_>, Vec<_>) =
            globals.into_iter().partition(|g| g.retain);
        // The bands' base takes the largest alignment they contain.
        let band_align = nonretain_globals
            .iter()
            .map(|g| g.align)
            .chain(retain_globals.iter().map(|g| g.align))
            .chain(retain_fields.iter().map(|r| r.align))
            .max()
            .unwrap_or(1);
        let globals_base = align_to(self.offset, band_align);
        let mut cursor = globals_base;
        let mut remap = FxHashMap::default();
        for g in &nonretain_globals {
            let addr = align_to(cursor, g.align);
            remap.insert(g.address, addr);
            cursor = addr + g.size;
        }
        // Retain globals begin the retain band (overlapping the globals band).
        let mut retain_base: Option<u32> = None;
        for g in &retain_globals {
            let addr = align_to(cursor, g.align);
            retain_base.get_or_insert(addr);
            remap.insert(g.address, addr);
            cursor = addr + g.size;
        }
        let globals_end = cursor; // globals band = non-retain + retain globals
        // Retain program-instances continue (and end) the retain band.
        for r in &retain_fields {
            let addr = align_to(cursor, r.align);
            retain_base.get_or_insert(addr);
            remap.insert(r.address, addr);
            cursor = addr + r.size;
        }
        self.offset = cursor;
        let retain_base = retain_base.unwrap_or(cursor);
        MemoryBands {
            globals_base,
            globals_size: globals_end - globals_base,
            retain_base,
            retain_size: cursor - retain_base,
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
