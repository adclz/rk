// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `[addresses]` in `config.toml`: how a located variable's number names its
//! bytes (`numbering`) and which byte of a wider value comes first
//! (`byte-order`). Each size counting in its own units, little-endian, unless
//! the workspace says otherwise.

use db::config_file::{AddressesConfig, ByteOrder, Numbering, parse_config};
use rk::workspace::init_db;

use super::{disable_stdlib, temp_workspace};
use crate::tests::codegen::execute_wasm;

const PROJECT: &str = "[project]\nname = \"T\"\nversion = \"0.0\"\n";

/// The two settings, each its default when left out, and a value of
/// neither refused.
#[test]
fn the_settings_read_as_written_or_as_today() {
    let read = |section: &str| parse_config(&format!("{PROJECT}{section}")).map(|c| c.addresses());
    assert_eq!(read("").unwrap(), AddressesConfig::default());
    assert_eq!(
        read("").unwrap(),
        AddressesConfig {
            numbering: Numbering::Width,
            byte_order: ByteOrder::Little,
        }
    );
    assert_eq!(
        read("[addresses]\nnumbering = \"byte\"\nbyte-order = \"big\"\n").unwrap(),
        AddressesConfig {
            numbering: Numbering::Byte,
            byte_order: ByteOrder::Big,
        }
    );
    assert_eq!(
        read("[addresses]\nnumbering = \"byte\"\n").unwrap(),
        AddressesConfig {
            numbering: Numbering::Byte,
            byte_order: ByteOrder::Little,
        }
    );
    let refused = read("[addresses]\nnumbering = \"word\"\n").unwrap_err();
    assert!(refused.to_string().contains("width"), "{refused}");
    assert!(read("[addresses]\norder = \"big\"\n").is_err());
}

/// An exported FUNCTION that writes two addresses and reads back two
/// narrower ones, which are their parts when the workspace counts bytes:
/// `%MB209` is byte 209 of `%MW208`, `%MW101` bytes 101 and 102 of
/// `%MD100`.
const PROBE: &str = "
{export}
FUNCTION byte_of_word : BYTE
    %MW208 := WORD#16#1234;
    byte_of_word := %MB209;
END_FUNCTION

{export}
FUNCTION word_of_dword : WORD
    %MD100 := DWORD#16#11223344;
    word_of_dword := %MW101;
END_FUNCTION
";

/// The module of a workspace holding [`PROBE`] under `section`.
fn built(section: &str) -> Vec<u8> {
    let config = format!("{PROJECT}{section}");
    let (_ws, root) = temp_workspace(&[("config.toml", &config), ("main.st", PROBE)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    rk::compiler::build_core_quiet(&db, &root, false)
        .expect("it builds")
        .0
}

/// Counted by width, `%MB209` and `%MW101` are cells of their own, which
/// nothing wrote; counted by byte, they are inside what was written, its
/// bytes little-endian.
#[test]
fn counting_bytes_puts_a_narrower_address_inside_a_wider_one() {
    let by_width = built("");
    assert_eq!(execute_wasm::<(), i32>(&by_width, "byte_of_word", ()), 0);
    assert_eq!(execute_wasm::<(), i32>(&by_width, "word_of_dword", ()), 0);

    let by_byte = built("[addresses]\nnumbering = \"byte\"\n");
    assert_eq!(execute_wasm::<(), i32>(&by_byte, "byte_of_word", ()), 0x12);
    assert_eq!(
        execute_wasm::<(), i32>(&by_byte, "word_of_dword", ()),
        0x2233
    );
}

/// Exported FUNCTIONs that write an address and read a narrower one inside
/// it, or the other way round, and one that writes a word through a
/// FUNCTION's `VAR_IN_OUT`, which takes its address. Which byte of a word
/// is which follows the workspace's byte order; reading and writing them
/// needs nothing more.
const ORDERED: &str = "
FUNCTION through : WORD
VAR_IN_OUT v : WORD; END_VAR
    v := WORD#16#ABCD;
    through := v;
END_FUNCTION

{export}
FUNCTION low_byte : BYTE
    %MW208 := WORD#16#1234;
    low_byte := %MB209;
END_FUNCTION

{export}
FUNCTION last_word : WORD
    %MD100 := DWORD#16#11223344;
    last_word := %MW102;
END_FUNCTION

{export}
FUNCTION bit_read : BOOL
    %MW208 := WORD#16#0020;
    bit_read := %MX209.5;
END_FUNCTION

{export}
FUNCTION byte_written : WORD
    %MW208 := WORD#0;
    %MB209 := BYTE#16#AB;
    byte_written := %MW208;
END_FUNCTION

{export}
FUNCTION bit_written : WORD
    %MW208 := WORD#0;
    %MX208.0 := TRUE;
    bit_written := %MW208;
END_FUNCTION

{export}
FUNCTION by_reference : BYTE
    %MW208 := WORD#0;
    through(v := %MW208);
    by_reference := %MB208;
END_FUNCTION
";

/// The module of a workspace holding `source` under `section`.
fn built_from(section: &str, source: &str) -> Vec<u8> {
    let config = format!("{PROJECT}{section}");
    let (_ws, root) = temp_workspace(&[("config.toml", &config), ("main.st", source)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    rk::compiler::build_core_quiet(&db, &root, false)
        .expect("it builds")
        .0
}

/// Counting bytes, the first byte of a word is its low byte when
/// little-endian and its high byte when big-endian, and so on for every
/// part: `%MB209` of `%MW208`, `%MW102` of `%MD100`, `%MX209.5` of
/// `%MW208`. A word written through a `VAR_IN_OUT` takes the area's order
/// too: the callee reads and writes it as any other value.
#[test]
fn the_byte_order_says_which_byte_of_a_value_is_which() {
    for (order, low, last, bit, byte, set, referenced) in [
        ("little", 0x12, 0x1122, 0, 0xAB00, 0x0001, 0xCD),
        ("big", 0x34, 0x3344, 1, 0x00AB, 0x0100, 0xAB),
    ] {
        let wasm = built_from(
            &format!("[addresses]\nnumbering = \"byte\"\nbyte-order = \"{order}\"\n"),
            ORDERED,
        );
        let run = |name: &str| execute_wasm::<(), i32>(&wasm, name, ());
        assert_eq!(run("low_byte"), low, "{order}: %MB209 of %MW208 := 16#1234");
        assert_eq!(
            run("last_word"),
            last,
            "{order}: %MW102 of %MD100 := 16#11223344"
        );
        assert_eq!(
            run("bit_read"),
            bit,
            "{order}: %MX209.5 of %MW208 := 16#0020"
        );
        assert_eq!(
            run("byte_written"),
            byte,
            "{order}: %MW208 after %MB209 := 16#AB"
        );
        assert_eq!(
            run("bit_written"),
            set,
            "{order}: %MW208 after %MX208.0 := TRUE"
        );
        assert_eq!(
            run("by_reference"),
            referenced,
            "{order}: %MB208 after a VAR_IN_OUT wrote 16#ABCD"
        );
    }
}

/// Counted by width, the byte order says the same: `%MX1.3` is bit 11 of
/// `%MW0` little-endian, and bit 3 big-endian.
#[test]
fn counted_by_width_the_byte_order_says_the_same() {
    let source = "
{export}
FUNCTION high_bit : BOOL
    %MW0 := WORD#16#0800;
    high_bit := %MX1.3;
END_FUNCTION
";
    for (order, set) in [("little", 1), ("big", 0)] {
        let wasm = built_from(&format!("[addresses]\nbyte-order = \"{order}\"\n"), source);
        assert_eq!(
            execute_wasm::<(), i32>(&wasm, "high_bit", ()),
            set,
            "{order}"
        );
    }
}

/// The `located-map` gives a host a part as its owner's cell and the bit
/// its value starts at in the owner's value, which the byte order moves:
/// a host reads and writes the owner's value as any other.
#[test]
fn the_located_map_places_a_part_by_the_byte_order() {
    let source = "
PROGRAM P
VAR
    word AT %QW208 : WORD;
    low AT %QB209 : BYTE;
    flag AT %QX209.5 : BOOL;
END_VAR
    word := WORD#1;
END_PROGRAM

CONFIGURATION C
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";
    for (order, byte_shift, bit_shift) in [("little", 8, 13), ("big", 0, 5)] {
        let wasm = built_from(
            &format!("[addresses]\nnumbering = \"byte\"\nbyte-order = \"{order}\"\n"),
            source,
        );
        let section =
            crate::tests::codegen::custom_section(&wasm, debug_format::LOCATED_MAP_SECTION)
                .expect("a located map");
        let map = debug_format::LocatedMap::from_msgpack(section).expect("it decodes");
        let entry = |address: &str| {
            map.entries
                .iter()
                .find(|e| e.address == address)
                .unwrap_or_else(|| panic!("{address} is in the map"))
        };
        let word = entry("%QW208");
        for (address, shift) in [("%QB209", byte_shift), ("%QX209.5", bit_shift)] {
            let part = entry(address);
            let of = part.part_of.as_ref().expect("a part");
            assert_eq!(of.owner, "%QW208", "{order}: {address}");
            assert_eq!(of.shift, shift, "{order}: {address}");
            assert_eq!(
                part.offset, word.offset,
                "{order}: {address} is in the word's cell"
            );
        }
    }
}
