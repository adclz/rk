use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::variable::LocationArea;
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
    /// Every configuration VAR_GLOBAL as allocated, for the host-visible band;
    /// `retain` globals sit in its overlap with the retain band.
    pub global_allocations: Vec<GlobalEntry>,
    /// Every located (`AT %…`) VAR_GLOBAL as allocated, for the three I/O
    /// bands.
    pub located_allocations: Vec<LocatedEntry>,
}

impl Default for MirMemoryLayout {
    fn default() -> Self {
        Self {
            offset: BUILTIN_RESERVED_FLOOR,
            allocations: Vec::new(),
            retain_allocations: Vec::new(),
            global_allocations: Vec::new(),
            located_allocations: Vec::new(),
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

/// A configuration VAR_GLOBAL, recorded as it is allocated. `retain` globals
/// are persisted and sit in the overlap of the globals and retain bands.
#[derive(Debug, Clone, Copy)]
pub struct GlobalEntry {
    pub name: Ident,
    pub address: u32,
    pub size: u32,
    pub align: u32,
    pub retain: bool,
}

/// A located (`AT %…`) VAR_GLOBAL, recorded as it is allocated. Its band is
/// the address's area; its place INSIDE that band is `(width, offsets,
/// address)`, so the layout depends only on which addresses exist and not on
/// the order the program happens to mention them — an unrelated edit must not
/// shift every address in the debug symbols.
#[derive(Debug, Clone)]
pub struct LocatedEntry {
    pub name: Ident,
    /// The address as written (`%IX0.1`): the key a host binds a channel to.
    pub address_text: String,
    pub area: LocationArea,
    /// Rank of the address's width letter: `X` < `B` < `W` < `D` < `L`.
    pub width_rank: u8,
    /// The numeric parts of the address, in order.
    pub offsets: Vec<u32>,
    /// Linear-memory address; pre-band-relocation as allocated, final once
    /// [`MirMemoryLayout::finalize_bands`] has returned it.
    pub address: u32,
    pub size: u32,
    pub align: u32,
}

/// The bands after relocation: `[globals_base, globals_size)` holds every
/// VAR_GLOBAL, `[retain_base, retain_size)` the retained set; retain
/// globals are the overlap. The three located bands sit below both, one per
/// area, each contiguous so a host copies a whole direction at once.
#[derive(Debug, Clone)]
pub struct MemoryBands {
    pub globals_base: u32,
    pub globals_size: u32,
    pub retain_base: u32,
    pub retain_size: u32,
    pub input_base: u32,
    pub input_size: u32,
    pub output_base: u32,
    pub output_size: u32,
    pub marker_base: u32,
    pub marker_size: u32,
    /// The located variables at their FINAL addresses, in band order.
    pub located: Vec<LocatedEntry>,
    /// Old address → in-band address; applied to every stored absolute address.
    pub remap: FxHashMap<u32, u32>,
    /// The retained VAR_GLOBALs at their FINAL (in-band) addresses — the
    /// retain-map builder emits one persistence range per entry.
    pub retain_globals: Vec<RetainEntry>,
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

    /// Register an allocated VAR_GLOBAL for the globals band; pure bookkeeping.
    pub fn record_global(
        &mut self,
        name: Ident,
        address: u32,
        size: u32,
        align: u32,
        retain: bool,
    ) {
        self.global_allocations.push(GlobalEntry {
            name,
            address,
            size,
            align,
            retain,
        });
    }

    /// Register an allocated located (`AT %…`) VAR_GLOBAL for its I/O band;
    /// pure bookkeeping.
    pub fn record_located(&mut self, entry: LocatedEntry) {
        self.located_allocations.push(entry);
    }

    /// Relocate every RETAIN and located variable into contiguous bands at
    /// the top of the arena; returns the bounds and the old→new remap. The
    /// original slots stay as holes.
    pub fn finalize_bands(&mut self) -> MemoryBands {
        let retain_fields = std::mem::take(&mut self.retain_allocations);
        let globals = std::mem::take(&mut self.global_allocations);
        let located = std::mem::take(&mut self.located_allocations);
        if globals.is_empty() && retain_fields.is_empty() && located.is_empty() {
            let p = self.offset;
            return MemoryBands {
                globals_base: p,
                globals_size: 0,
                retain_base: p,
                retain_size: 0,
                input_base: p,
                input_size: 0,
                output_base: p,
                output_size: 0,
                marker_base: p,
                marker_size: 0,
                located: Vec::new(),
                remap: FxHashMap::default(),
                retain_globals: Vec::new(),
            };
        }
        // `[ %I | %Q | %M | non-retain globals | retain globals |
        //    retain program-instances ]`, contiguous at the arena top.
        let (retain_globals, nonretain_globals): (Vec<_>, Vec<_>) =
            globals.into_iter().partition(|g| g.retain);
        // The bands' base takes the largest alignment they contain.
        let band_align = nonretain_globals
            .iter()
            .map(|g| g.align)
            .chain(retain_globals.iter().map(|g| g.align))
            .chain(retain_fields.iter().map(|r| r.align))
            .chain(located.iter().map(|l| l.align))
            .max()
            .unwrap_or(1);
        let mut cursor = align_to(self.offset, band_align);
        let mut remap = FxHashMap::default();

        // The located bands come first, one per area. Sorted by the ADDRESS,
        // never by allocation order, so which addresses exist is the only
        // thing the layout depends on.
        let mut located = located;
        located.sort_by(|a, b| {
            (a.area, a.width_rank, &a.offsets, &a.address_text).cmp(&(
                b.area,
                b.width_rank,
                &b.offsets,
                &b.address_text,
            ))
        });
        let mut relocated_located = Vec::with_capacity(located.len());
        let mut area_bands = [(cursor, 0u32); 3];
        for (slot, area) in area_bands.iter_mut().zip([
            LocationArea::Input,
            LocationArea::Output,
            LocationArea::Marker,
        ]) {
            let mut base: Option<u32> = None;
            for entry in located.iter().filter(|e| e.area == area) {
                let addr = align_to(cursor, entry.align);
                base.get_or_insert(addr);
                remap.insert(entry.address, addr);
                relocated_located.push(LocatedEntry {
                    address: addr,
                    ..entry.clone()
                });
                cursor = addr + entry.size;
            }
            let base = base.unwrap_or(cursor);
            *slot = (base, cursor - base);
        }

        let globals_base = align_to(cursor, band_align);
        cursor = globals_base;
        for g in &nonretain_globals {
            let addr = align_to(cursor, g.align);
            remap.insert(g.address, addr);
            cursor = addr + g.size;
        }
        // Retain globals begin the retain band (overlapping the globals band).
        let mut retain_base: Option<u32> = None;
        let mut relocated_retain_globals = Vec::new();
        for g in &retain_globals {
            let addr = align_to(cursor, g.align);
            retain_base.get_or_insert(addr);
            remap.insert(g.address, addr);
            relocated_retain_globals.push(RetainEntry {
                name: g.name,
                address: addr,
                size: g.size,
                align: g.align,
            });
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
            input_base: area_bands[0].0,
            input_size: area_bands[0].1,
            output_base: area_bands[1].0,
            output_size: area_bands[1].1,
            marker_base: area_bands[2].0,
            marker_size: area_bands[2].1,
            located: relocated_located,
            remap,
            retain_globals: relocated_retain_globals,
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
