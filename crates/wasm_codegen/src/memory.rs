//! Memory layout management for WASM linear memory.
//!
//! This module handles static allocation of arrays, structs, and other
//! memory-resident values in WebAssembly linear memory.

use hir::hir_def::interned::identifier::Ident;
use rustc_hash::FxHashMap;

/// Manages memory layout for static allocation in WASM linear memory.
#[derive(Debug, Clone)]
pub struct MemoryLayout {
    /// Next available memory address (byte offset).
    next_addr: u32,

    /// Allocations: variable name → allocation info.
    allocations: FxHashMap<Ident, MemoryAllocation>,
}

/// Information about an allocated memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryAllocation {
    /// Base address in linear memory (byte offset).
    pub address: u32,

    /// Size in bytes.
    pub size: u32,

    /// Alignment requirement.
    pub align: u32,
}

impl MemoryLayout {
    /// Create a new memory layout manager.
    ///
    /// Memory allocation starts at address 0.
    pub fn new() -> Self {
        Self {
            next_addr: 0,
            allocations: FxHashMap::default(),
        }
    }

    /// Allocate space for a variable.
    ///
    /// This function:
    /// 1. Aligns `next_addr` to the specified alignment
    /// 2. Assigns the aligned address to the variable
    /// 3. Increments `next_addr` by `size`
    /// 4. Returns the allocated address
    ///
    /// # Arguments
    /// * `name` - Variable identifier
    /// * `size` - Size in bytes
    /// * `align` - Alignment requirement (must be power of 2)
    ///
    /// # Returns
    /// The base address of the allocated region.
    ///
    /// # Example
    /// ```ignore
    /// let mut layout = MemoryLayout::new();
    /// let addr = layout.allocate(my_var, 40, 4);  // Allocate 40 bytes, 4-byte aligned
    /// ```
    pub fn allocate(&mut self, name: Ident, size: u32, align: u32) -> u32 {
        // Round up to next multiple of align
        let address = align_to(self.next_addr, align);

        // Store allocation info
        self.allocations.insert(
            name,
            MemoryAllocation {
                address,
                size,
                align,
            },
        );

        // Advance next_addr
        self.next_addr = address + size;

        address
    }

    /// Get allocation info for a variable.
    ///
    /// # Arguments
    /// * `name` - Variable identifier
    ///
    /// # Returns
    /// Allocation info if the variable was allocated, None otherwise.
    pub fn get(&self, name: &Ident) -> Option<MemoryAllocation> {
        self.allocations.get(name).copied()
    }

    /// Get the total memory size allocated so far.
    ///
    /// This is used to determine the minimum size for the WASM memory section.
    ///
    /// # Returns
    /// Total size in bytes.
    pub fn total_size(&self) -> u32 {
        self.next_addr
    }

    /// Get the number of variables allocated.
    pub fn allocation_count(&self) -> usize {
        self.allocations.len()
    }
}

impl Default for MemoryLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: align offset to specified alignment.
///
/// Returns the smallest value >= offset that is a multiple of align.
///
/// # Arguments
/// * `offset` - Current offset
/// * `align` - Alignment requirement (must be power of 2)
///
/// # Returns
/// Aligned offset.
///
/// # Example
/// ```ignore
/// assert_eq!(align_to(5, 4), 8);   // 5 rounded up to next multiple of 4
/// assert_eq!(align_to(8, 4), 8);   // 8 is already aligned
/// assert_eq!(align_to(0, 4), 0);   // 0 is aligned to anything
/// ```
fn align_to(offset: u32, align: u32) -> u32 {
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
        assert_eq!(align_to(9, 8), 16);
    }

    // Note: Tests for MemoryLayout allocation require a database to create Ident values.
    // These will be tested through integration tests that have access to a RootDatabase.
}
