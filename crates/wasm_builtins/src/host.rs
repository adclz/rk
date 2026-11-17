//! A list an import gives, in blocks the host writes where the guest says:
//! the list's elements, then each string they hold. While a call that
//! gives a list runs, `cabi_import_realloc` asks [`rk_host_list_block`]
//! where each block goes; after it, [`rk_host_list_end`] checks what came
//! and copies the scalar fields of the elements into the caller's arrays.
//!
//! The call describes the caller's arrays in a state of 32-bit words, laid
//! out as below and as `wasm_codegen::control` writes it.

use crate::trap::rk_raise;

/// 1 while a call that gives a list runs.
const ARMED: usize = 0;
/// The size of one element, as the canonical ABI lays it out.
const ELEMENT_SIZE: usize = 1;
/// How many fields an element has, one array each.
const FIELDS: usize = 2;
/// How many of them are strings.
const STRINGS: usize = 3;
/// How many elements the host gave.
const COUNT: usize = 4;
/// Where the elements are.
const ELEMENT_AT: usize = 5;
/// Whether the block of the elements came.
const ELEMENT_SEEN: usize = 6;
/// How many strings came since.
const STRINGS_SEEN: usize = 7;
/// Whether the host wrote the elements into the caller's array itself.
const DIRECT: usize = 8;
/// Whether a string was longer than its STRING.
const TOO_LONG: usize = 9;
/// Where a block nobody reads goes, and how long it may be: the place every
/// such block of the call lands on, past what the call keeps.
const REFUSED_AT: usize = 10;
const REFUSED_SIZE: usize = 11;
/// Where the fields start, and the words of each: whether it is a string,
/// its offset and size in an element, then its array: where it is, the
/// bytes from one element to the next, the capacity of a STRING element,
/// and how many elements it has.
const FIELD_AT: usize = 12;
const FIELD_WORDS: usize = 7;
const IS_STRING: usize = 0;
const OFFSET: usize = 1;
const SIZE: usize = 2;
const BASE: usize = 3;
const STRIDE: usize = 4;
const CAPACITY: usize = 5;
const LENGTH: usize = 6;

unsafe fn get(state: *const u32, at: usize) -> u32 {
    unsafe { state.add(at).read() }
}

unsafe fn put(state: *mut u32, at: usize, value: u32) {
    unsafe { state.add(at).write(value) }
}

fn field(index: usize, word: usize) -> usize {
    FIELD_AT + index * FIELD_WORDS + word
}

/// Where a block nobody reads lands: the end raises, the list or one of its
/// strings being longer than the call takes. Such a block longer than its
/// place traps, as the memory never grows.
unsafe fn refuse(state: *const u32, size: u32) -> u32 {
    unsafe {
        if size > get(state, REFUSED_SIZE) {
            core::arch::wasm32::unreachable()
        }
        get(state, REFUSED_AT)
    }
}

/// Where the host's next block goes while a call that gives a list runs, or
/// 0 for the module's own `cabi_realloc`. The first block holds the list's
/// elements: in the caller's array when the list is of scalars laid out as
/// the array's, and fits; with `cabi_realloc` when the call keeps the list
/// otherwise. Each block after it is a string, in the order the elements
/// hold them: in its element of the caller's STRING array, its length
/// written first, when it fits. A block the call does not keep is refused.
#[unsafe(no_mangle)]
pub extern "C" fn rk_host_list_block(state: *mut u32, size: u32) -> u32 {
    unsafe {
        if get(state, ELEMENT_SEEN) == 0 {
            put(state, ELEMENT_SEEN, 1);
            let element = get(state, ELEMENT_SIZE).max(1);
            let count = size / element;
            put(state, COUNT, count);
            let fields = get(state, FIELDS) as usize;
            if (0..fields).any(|index| count > get(state, field(index, LENGTH))) {
                return refuse(state, size);
            }
            let direct =
                fields == 1 && get(state, STRINGS) == 0 && get(state, field(0, STRIDE)) == element;
            if !direct {
                return 0;
            }
            put(state, DIRECT, 1);
            let at = get(state, field(0, BASE));
            put(state, ELEMENT_AT, at);
            return at;
        }
        let strings = get(state, STRINGS);
        if strings == 0 {
            return refuse(state, size);
        }
        let seen = get(state, STRINGS_SEEN);
        put(state, STRINGS_SEEN, seen + 1);
        let (element, nth) = (seen / strings, seen % strings);
        let mut string = 0;
        for index in 0..get(state, FIELDS) as usize {
            if get(state, field(index, IS_STRING)) == 0 {
                continue;
            }
            if string != nth {
                string += 1;
                continue;
            }
            // Past the array: the end raises, the list being longer.
            if element >= get(state, field(index, LENGTH)) {
                return refuse(state, size);
            }
            if size > get(state, field(index, CAPACITY)) {
                put(state, TOO_LONG, 1);
                return refuse(state, size);
            }
            let at = get(state, field(index, BASE)) + element * get(state, field(index, STRIDE));
            (at as *mut u32).write(size);
            return at + 4;
        }
        refuse(state, size)
    }
}

/// Where the module's own `cabi_realloc` put a block [`rk_host_list_block`]
/// left to it: kept when it is the block of the elements, which the end
/// copies the scalar fields out of.
#[unsafe(no_mangle)]
pub extern "C" fn rk_host_list_placed(state: *mut u32, at: u32) {
    unsafe {
        let elements = get(state, STRINGS_SEEN) == 0 && get(state, DIRECT) == 0;
        if elements && get(state, ELEMENT_AT) == 0 {
            put(state, ELEMENT_AT, at);
        }
    }
}

/// The end of a call that gives a list: raises when the list is longer
/// than one of the caller's arrays, or one of its strings longer than its
/// STRING; otherwise copies each scalar field of each element into its
/// array, unless the host wrote the elements there itself.
#[unsafe(no_mangle)]
pub extern "C" fn rk_host_list_end(state: *mut u32) {
    unsafe {
        put(state, ARMED, 0);
        let count = get(state, COUNT);
        let fields = get(state, FIELDS) as usize;
        for index in 0..fields {
            if count > get(state, field(index, LENGTH)) {
                rk_raise("a list the host gave is longer than its array");
            }
        }
        if get(state, TOO_LONG) != 0 {
            rk_raise("a string the host gave is longer than its STRING");
        }
        if get(state, DIRECT) != 0 {
            return;
        }
        let from = get(state, ELEMENT_AT) as *const u8;
        let element = get(state, ELEMENT_SIZE) as usize;
        for index in 0..fields {
            if get(state, field(index, IS_STRING)) != 0 {
                continue;
            }
            let offset = get(state, field(index, OFFSET)) as usize;
            let size = get(state, field(index, SIZE)) as usize;
            let base = get(state, field(index, BASE)) as *mut u8;
            let stride = get(state, field(index, STRIDE)) as usize;
            for at in 0..count as usize {
                core::ptr::copy_nonoverlapping(
                    from.add(at * element + offset),
                    base.add(at * stride),
                    size,
                );
            }
        }
    }
}
