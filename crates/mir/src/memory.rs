// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::variable::LocationArea;
use rustc_hash::{FxHashMap, FxHashSet};

/// Bytes reserved at the bottom of linear memory before any IEC allocation:
/// the grafted `wasm_builtins` use `[0, 8192)` as their shadow stack and
/// `[8192, 9008)` for their data segments; rounded up to 16 KiB.
pub const BUILTIN_RESERVED_FLOOR: u32 = 16_384;

/// Memory layout for the entire module - fully resolved during MIR lowering.
#[derive(Debug, Clone)]
pub struct MirMemoryLayout {
    /// The next available address, counted past the last one a module has,
    /// so a layout that does not fit can say by how much (see
    /// [`Self::end`]).
    offset: u64,
    /// The furthest `offset` went, before the bands took back the slots
    /// their variables were first allocated in.
    peak: u64,
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
    /// The bytes of the frame being laid out, while a recursive function is
    /// lowered: its locals go there instead of at static addresses.
    frame: Option<u32>,
}

impl Default for MirMemoryLayout {
    fn default() -> Self {
        Self {
            offset: u64::from(BUILTIN_RESERVED_FLOOR),
            peak: u64::from(BUILTIN_RESERVED_FLOOR),
            allocations: Vec::new(),
            retain_allocations: Vec::new(),
            global_allocations: Vec::new(),
            located_allocations: Vec::new(),
            frame: None,
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
    /// The address as HIR decoded it. Its area is the band, and its width
    /// and levels place the cell inside it.
    pub located: hir::hir_def::pous::variable::LocatedAddress,
    /// Declared `RETAIN`. Only `%M` can be: an input image restored at
    /// startup would run the first scan on the last power cycle's values,
    /// and `%I`/`%Q` are refused at check (E1420).
    pub retain: bool,
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

    /// Allocate `size` bytes at `align`; returns the address. A part that
    /// ends past the last address a module has gets address 0: lowering
    /// refuses the layout once everything is counted, and no module is
    /// emitted with it.
    pub fn allocate(&mut self, name: Ident, size: u32, align: u32, kind: MirAllocKind) -> u32 {
        let start = align_to_u64(self.offset, align);
        self.offset = start + u64::from(size);
        self.peak = self.peak.max(self.offset);
        let address = within(start, self.offset);
        self.allocations.push(MirAllocation {
            name,
            address,
            size,
            align,
            kind,
        });
        address
    }

    /// Where the layout ends, past the last address a module has when it
    /// does not fit. A layout that went past it before the bands gave back
    /// their slots handed out addresses that stand for nothing, and ends
    /// there.
    pub fn end(&self) -> u64 {
        match self.peak > u64::from(u32::MAX) {
            true => self.peak,
            false => self.offset,
        }
    }

    /// Lay out the locals that follow in a frame, for a function that may
    /// call itself; [`Self::end_frame`] goes back to static addresses.
    pub fn begin_frame(&mut self) {
        self.frame = Some(0);
    }

    /// The frame begun by [`Self::begin_frame`], closed; `None` when none was.
    pub fn end_frame(&mut self) -> Option<crate::function::MirFrame> {
        use crate::function::{FRAME_ALIGN, MirFrame};
        self.frame.take().map(|size| MirFrame {
            size: align_to(size, FRAME_ALIGN),
        })
    }

    /// `size` bytes at `align` in the frame being laid out; `None` outside
    /// one. Each part starts at a multiple of [`FRAME_ALIGN`] and takes its
    /// [`slot`], so the frame is the size HIR plans for it, in whatever
    /// order its parts come. A value of no size, an instance of a block with
    /// no variables, still takes a slot: the code generator gives a function
    /// a frame base only for a frame that pushes something, and a frame
    /// local without one is a panic there.
    ///
    /// [`FRAME_ALIGN`]: crate::function::FRAME_ALIGN
    /// [`slot`]: hir::hir_ty::frame::slot
    pub fn allocate_in_frame(&mut self, size: u32, align: u32) -> Option<u32> {
        let used = self.frame.as_mut()?;
        debug_assert!(align <= crate::function::FRAME_ALIGN);
        let offset = *used;
        let slot = hir::hir_ty::frame::slot(hir::hir_ty::layout::Layout {
            size: u64::from(size),
            align,
        });
        *used = offset + slot as u32;
        Some(offset)
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

    /// Relocate every RETAIN and located variable, and every VAR_GLOBAL, into
    /// contiguous bands at the top of the arena; returns the bounds and the
    /// old→new remap. The slots they were first allocated in are given back:
    /// the rest of the arena is laid out again without them, each address
    /// that moves going through the remap too, so a variable takes its size
    /// once.
    pub fn finalize_bands(&mut self) -> MemoryBands {
        let retain_fields = std::mem::take(&mut self.retain_allocations);
        let globals = std::mem::take(&mut self.global_allocations);
        let located = std::mem::take(&mut self.located_allocations);
        if globals.is_empty() && retain_fields.is_empty() && located.is_empty() {
            let p = within(self.offset, self.offset);
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
        // `[ %I | %Q | %M transient | %M retained | retain program-instances |
        //    retain globals | non-retain globals ]`, contiguous at the arena
        // top. Everything retained is in the middle, so the retain band holds
        // nothing transient but the holes inside a retained instance — which
        // are per-field by design, and which the retain MAP names.
        //
        // A retained `%M` cell has to be in TWO bands at once: its own, which
        // a host copies whole, and the retain band, which a power cycle
        // preserves. It can only be in both if the retain band starts inside
        // `%M`, so the retained cells sit at the end of that area and
        // `retain_base` lands on the first of them. The band then also spans
        // the non-retain globals, which is what the retain MAP is for: it
        // names the ranges that actually persist, and everything else in the
        // band stays transient.
        let banded: FxHashSet<(Ident, u32)> = retain_fields
            .iter()
            .map(|r| (r.name, r.address))
            .chain(globals.iter().map(|g| (g.name, g.address)))
            .chain(located.iter().map(|l| (l.name, l.address)))
            .collect();
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
        let mut remap = FxHashMap::default();
        let mut cursor = u64::from(BUILTIN_RESERVED_FLOOR);
        for allocation in &mut self.allocations {
            if banded.contains(&(allocation.name, allocation.address)) {
                continue;
            }
            let start = align_to_u64(cursor, allocation.align);
            cursor = start + u64::from(allocation.size);
            let address = within(start, cursor);
            // A part of no size holds nothing to move, and shares its address
            // with the part after it, whose entry it must not take.
            if allocation.size > 0 && address != allocation.address {
                remap.insert(allocation.address, address);
            }
            allocation.address = address;
        }
        let mut cursor = align_to_u64(cursor, band_align);

        // The retain band may begin inside `%M`, so both are in hand before
        // the located walk rather than after it.
        let mut retain_base: Option<u64> = None;
        let mut relocated_retain_globals = Vec::new();

        // The located bands come first, one per area. Sorted by the ADDRESS,
        // never by allocation order, so which addresses exist is the only
        // thing the layout depends on — with the retained cells of an area
        // last, so they can end it and the retain band can start there.
        let mut located = located;
        located.sort_by(|a, b| {
            (
                a.located.area,
                a.retain,
                a.located.width,
                &a.located.levels,
                &a.address_text,
            )
                .cmp(&(
                    b.located.area,
                    b.retain,
                    b.located.width,
                    &b.located.levels,
                    &b.address_text,
                ))
        });
        let mut relocated_located = Vec::with_capacity(located.len());
        let mut area_bands = [(cursor, 0u64); 3];
        for (slot, area) in area_bands.iter_mut().zip([
            LocationArea::Input,
            LocationArea::Output,
            LocationArea::Marker,
        ]) {
            let mut base: Option<u64> = None;
            for entry in located.iter().filter(|e| e.located.area == area) {
                let start = align_to_u64(cursor, entry.align);
                cursor = start + u64::from(entry.size);
                let addr = within(start, cursor);
                base.get_or_insert(start);
                remap.insert(entry.address, addr);
                if entry.retain {
                    retain_base.get_or_insert(start);
                    relocated_retain_globals.push(RetainEntry {
                        name: entry.name,
                        address: addr,
                        size: entry.size,
                        align: entry.align,
                    });
                }
                relocated_located.push(LocatedEntry {
                    address: addr,
                    ..entry.clone()
                });
            }
            let base = base.unwrap_or(cursor);
            *slot = (base, cursor - base);
        }

        // Retain program-instances continue the retain band. They come
        // BEFORE the globals so the band can close on the retained globals:
        // with the non-retain ones in between, the band would enclose a whole
        // transient variable, and a host that restored the band rather than
        // the map's ranges would bring it back from the last power cycle
        // instead of its initializer.
        for r in &retain_fields {
            let start = align_to_u64(cursor, r.align);
            cursor = start + u64::from(r.size);
            retain_base.get_or_insert(start);
            remap.insert(r.address, within(start, cursor));
        }
        // The globals band opens on the retained globals, which are also
        // where the retain band ends: the two overlap on exactly them.
        let globals_base = align_to_u64(cursor, band_align);
        cursor = globals_base;
        for g in &retain_globals {
            let start = align_to_u64(cursor, g.align);
            cursor = start + u64::from(g.size);
            let addr = within(start, cursor);
            retain_base.get_or_insert(start);
            remap.insert(g.address, addr);
            relocated_retain_globals.push(RetainEntry {
                name: g.name,
                address: addr,
                size: g.size,
                align: g.align,
            });
        }
        let retain_end = cursor;
        for g in &nonretain_globals {
            let start = align_to_u64(cursor, g.align);
            cursor = start + u64::from(g.size);
            remap.insert(g.address, within(start, cursor));
        }
        let globals_end = cursor; // globals band = retain + non-retain globals
        self.offset = cursor;
        let retain_base = retain_base.unwrap_or(retain_end);
        // A band past the last address is refused with the layout; its
        // bounds are 0 meanwhile.
        let band = |base: u64, end: u64| match within(base, end) {
            0 => (0, 0),
            base32 => (base32, within(end - base, end)),
        };
        let (globals_base, globals_size) = band(globals_base, globals_end);
        let (retain_base, retain_size) = band(retain_base, retain_end);
        let [input, output, marker] = area_bands.map(|(base, size)| band(base, base + size));
        MemoryBands {
            globals_base,
            globals_size,
            retain_base,
            retain_size,
            input_base: input.0,
            input_size: input.1,
            output_base: output.0,
            output_size: output.1,
            marker_base: marker.0,
            marker_size: marker.1,
            located: relocated_located,
            remap,
            retain_globals: relocated_retain_globals,
        }
    }

    /// Total memory size used (useful for WASM memory section). Lowering
    /// refuses a layout past the last address before anything reads it.
    pub fn total_size(&self) -> u32 {
        within(self.offset, self.offset)
    }
}

/// `value` when the part it starts ends at or before the last address a
/// module has, `end` being where it ends; 0 otherwise, which no module is
/// emitted with.
fn within(value: u64, end: u64) -> u32 {
    match u32::try_from(end) {
        Ok(_) => value as u32,
        Err(_) => 0,
    }
}

/// [`align_to`] past the 32 bits of an address.
fn align_to_u64(offset: u64, align: u32) -> u64 {
    let align = u64::from(align.max(1));
    offset.div_ceil(align) * align
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
