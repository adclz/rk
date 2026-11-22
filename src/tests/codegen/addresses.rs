// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `[addresses]` in `config.toml`: how a located variable's number names its
//! bytes (`numbering`) and which byte of a wider value comes first
//! (`byte-order`), which decide where each address is stored and what it
//! reads. Each size counts in its own units, little-endian, unless the
//! workspace says otherwise.

use crate::tests::codegen::{
    TestPlc, compile_to_mir_and_wasm, custom_section, execute_wasm, expect_section, with_db,
};
use crate::tests::utils::with_config;
use debug_format::{DebugInfo, ScheduleManifest, SingleBit, VarValue};
use rstest::*;

const BY_BYTE: &str = "[addresses]\nnumbering = \"byte\"\n";
const BY_BYTE_BIG: &str = "[addresses]\nnumbering = \"byte\"\nbyte-order = \"big\"\n";

/// The MIR and the module of `source`, in a workspace whose `config.toml`
/// holds `section`.
fn built(db: &mut db::RootDatabase, section: &str, source: &str) -> (mir::MirModule, Vec<u8>) {
    let _config = with_config(db, section);
    compile_to_mir_and_wasm(db, source)
}

/// The `located-map` of a module.
fn located_map(wasm: &[u8]) -> debug_format::LocatedMap {
    let section = custom_section(wasm, debug_format::LOCATED_MAP_SECTION).expect("a located map");
    debug_format::LocatedMap::from_msgpack(section).expect("it decodes")
}

/// The entry of `address` in `map`.
fn entry<'m>(map: &'m debug_format::LocatedMap, address: &str) -> &'m debug_format::LocatedVar {
    map.entries
        .iter()
        .find(|e| e.address == address)
        .unwrap_or_else(|| panic!("{address} is in the map"))
}

// ---------------------------------------------------------------------------
// Numbering: an address of a wider size counted in bytes
// ---------------------------------------------------------------------------

/// FUNCTIONs that write two addresses and read back two narrower ones,
/// which are their parts when the workspace counts bytes: `%MB209` is byte
/// 209 of `%MW208`, `%MW101` bytes 101 and 102 of `%MD100`.
const PROBE: &str = "
FUNCTION byte_of_word : BYTE
    %MW208 := WORD#16#1234;
    byte_of_word := %MB209;
END_FUNCTION

FUNCTION word_of_dword : WORD
    %MD100 := DWORD#16#11223344;
    word_of_dword := %MW101;
END_FUNCTION
";

/// Counted by width, `%MB209` and `%MW101` are cells of their own, which
/// nothing wrote; counted by byte, they are inside what was written, its
/// bytes little-endian.
#[rstest]
#[case::by_width("", 0, 0)]
#[case::by_byte(BY_BYTE, 0x12, 0x2233)]
fn counting_bytes_puts_a_narrower_address_inside_a_wider_one(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] byte: i32,
    #[case] word: i32,
) {
    let (_, wasm) = built(&mut with_db, section, PROBE);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "byte_of_word", ()), byte);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "word_of_dword", ()), word);
}

// ---------------------------------------------------------------------------
// Byte order: which byte of a value is which address
// ---------------------------------------------------------------------------

/// FUNCTIONs that write an address and read a narrower one inside it, or
/// the other way round, and one that writes a word through a FUNCTION's
/// `VAR_IN_OUT`, which takes its address. Which byte of a word is which
/// follows the workspace's byte order; reading and writing them needs
/// nothing more.
const ORDERED: &str = "
FUNCTION through : WORD
VAR_IN_OUT v : WORD; END_VAR
    v := WORD#16#ABCD;
    through := v;
END_FUNCTION

FUNCTION through_byte : BYTE
VAR_IN_OUT v : BYTE; END_VAR
    v := BYTE#16#AB;
    through_byte := v;
END_FUNCTION

FUNCTION byte_209 : BYTE
    %MW208 := WORD#16#1234;
    byte_209 := %MB209;
END_FUNCTION

FUNCTION last_word : WORD
    %MD100 := DWORD#16#11223344;
    last_word := %MW102;
END_FUNCTION

FUNCTION bit_read : BOOL
    %MW208 := WORD#16#0020;
    bit_read := %MX209.5;
END_FUNCTION

FUNCTION byte_written : WORD
    %MW208 := WORD#0;
    %MB209 := BYTE#16#AB;
    byte_written := %MW208;
END_FUNCTION

FUNCTION bit_written : WORD
    %MW208 := WORD#0;
    %MX208.0 := TRUE;
    bit_written := %MW208;
END_FUNCTION

FUNCTION by_reference : BYTE
    %MW208 := WORD#0;
    through(v := %MW208);
    by_reference := %MB208;
END_FUNCTION

FUNCTION part_by_reference : WORD
    %MW208 := WORD#0;
    through_byte(v := %MB209);
    part_by_reference := %MW208;
END_FUNCTION
";

/// Counting bytes, the first byte of a word is its low byte when
/// little-endian and its high byte when big-endian, and so on for every
/// part: `%MB209` of `%MW208`, `%MW102` of `%MD100`, `%MX209.5` of
/// `%MW208`. A word written through a `VAR_IN_OUT` takes the area's order
/// too: the callee reads and writes it as any other value. A part passed to
/// one is its own bytes, where the order puts them in the cell.
#[rstest]
#[case::little(BY_BYTE, [0x12, 0x1122, 0, 0xAB00, 0x0001, 0xCD, 0xAB00])]
#[case::big(BY_BYTE_BIG, [0x34, 0x3344, 1, 0x00AB, 0x0100, 0xAB, 0x00AB])]
fn the_byte_order_says_which_byte_of_a_value_is_which(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] values: [i32; 7],
) {
    let (_, wasm) = built(&mut with_db, section, ORDERED);
    let run = |name: &str| execute_wasm::<(), i32>(&wasm, name, ());
    let [low, last, bit, byte, set, referenced, part] = values;
    assert_eq!(run("byte_209"), low, "%MB209 of %MW208 := 16#1234");
    assert_eq!(run("last_word"), last, "%MW102 of %MD100 := 16#11223344");
    assert_eq!(run("bit_read"), bit, "%MX209.5 of %MW208 := 16#0020");
    assert_eq!(run("byte_written"), byte, "%MW208 after %MB209 := 16#AB");
    assert_eq!(run("bit_written"), set, "%MW208 after %MX208.0 := TRUE");
    assert_eq!(
        run("by_reference"),
        referenced,
        "%MB208 after a VAR_IN_OUT wrote 16#ABCD"
    );
    assert_eq!(
        run("part_by_reference"),
        part,
        "%MW208 after a VAR_IN_OUT given %MB209 wrote 16#AB"
    );
}

/// Counted by width, the byte order says the same: `%MX1.3` is bit 11 of
/// `%MW0` little-endian, and bit 3 big-endian.
#[rstest]
#[case::little("[addresses]\nbyte-order = \"little\"\n", 1)]
#[case::big("[addresses]\nbyte-order = \"big\"\n", 0)]
fn counted_by_width_the_byte_order_says_the_same(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] set: i32,
) {
    let source = "
FUNCTION high_bit : BOOL
    %MW0 := WORD#16#0800;
    high_bit := %MX1.3;
END_FUNCTION
";
    let (_, wasm) = built(&mut with_db, section, source);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "high_bit", ()), set);
}

/// The `located-map` gives a host a part as its owner's cell and the bit
/// its value starts at in the owner's value, which the byte order moves:
/// a host reads and writes the owner's value as any other.
#[rstest]
#[case::little(BY_BYTE, 8, 13)]
#[case::big(BY_BYTE_BIG, 0, 5)]
fn the_located_map_places_a_part_by_the_byte_order(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] byte_shift: u16,
    #[case] bit_shift: u16,
) {
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
    let (_, wasm) = built(&mut with_db, section, source);
    let map = located_map(&wasm);
    let word = entry(&map, "%QW208");
    for (address, shift) in [("%QB209", byte_shift), ("%QX209.5", bit_shift)] {
        let part = entry(&map, address);
        let of = part.part_of.as_ref().expect("a part");
        assert_eq!(of.owner, "%QW208", "{address}");
        assert_eq!(of.shift, shift, "{address}");
        assert_eq!(part.offset, word.offset, "{address} is in the word's cell");
    }
}

/// A SINGLE on a part reads the bit of the cell the byte order says: with
/// `%IW208` declared, `%IX209.5` is in the second byte of the word
/// little-endian and in the first big-endian, and a host writing the word
/// raises it with the bit that is `%IX209.5` in its order. So it is when
/// the word shares a byte with `%IW207`, stored in that one's cell or the
/// other way round.
#[rstest]
#[case::little(BY_BYTE, "", 1, 0x2000)]
#[case::big(BY_BYTE_BIG, "", 0, 0x0020)]
#[case::little_shared(BY_BYTE, "a AT %IW207 : WORD; ", 1, 0x2000)]
#[case::big_shared(BY_BYTE_BIG, "a AT %IW207 : WORD; ", 0, 0x0020)]
fn a_single_on_a_part_reads_the_bit_the_order_says(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] beside: &str,
    #[case] offset: u32,
    #[case] word: u16,
) {
    let source = "
PROGRAM Count VAR n : DINT; END_VAR n := n + 1; END_PROGRAM

CONFIGURATION Cfg
    VAR_GLOBAL BESIDE w AT %IW208 : WORD; END_VAR
    RESOURCE Res ON CPU
        TASK Ev(SINGLE := %IX209.5, PRIORITY := 1);
        PROGRAM Runs WITH Ev : Count;
    END_RESOURCE
END_CONFIGURATION
";
    let source = source.replace("BESIDE ", beside);
    let (_, wasm) = built(&mut with_db, section, &source);
    let manifest =
        ScheduleManifest::from_msgpack(expect_section(&wasm, debug_format::SCHEDULE_SECTION))
            .expect("it decodes");
    let info = DebugInfo::from_wasm(&wasm);
    let cell = info.resolve("w").expect("w").address;
    let single = manifest.tasks.iter().find(|t| t.name == "Ev").expect("Ev");
    assert_eq!(
        single.single,
        Some(SingleBit {
            addr: cell + offset,
            bit: 5
        })
    );

    let runs = |plc: &TestPlc| match plc.read_var(&info, "Runs.n") {
        Some(VarValue::I32(n)) => n,
        other => panic!("`Runs.n` is {other:?}"),
    };
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.write_located("%IW208", &(!word).to_le_bytes()).unwrap();
    plc.run(2).expect("scans");
    assert_eq!(runs(&plc), 0, "every other bit set");
    plc.write_located("%IW208", &word.to_le_bytes()).unwrap();
    plc.scan().expect("scan");
    assert_eq!(runs(&plc), 1, "the word raised it");
}

/// A declared part reads, through its debug symbol and through the located
/// map, the byte of its owner the order says: `%QB209` of `%QW208 :=
/// 16#1234` is 16#12 little-endian and 16#34 big-endian.
#[rstest]
#[case::little(BY_BYTE, 0x12)]
#[case::big(BY_BYTE_BIG, 0x34)]
fn a_declared_part_reads_the_byte_the_order_says(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] byte: u8,
) {
    let source = "
PROGRAM P
VAR
    word AT %QW208 : WORD;
    low AT %QB209 : BYTE;
END_VAR
    word := WORD#16#1234;
END_PROGRAM

CONFIGURATION C
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";
    let (_, wasm) = built(&mut with_db, section, source);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("a scan");
    assert_eq!(plc.read_var(&info, "Main.low"), Some(VarValue::U8(byte)));
    // The host's reading of a part, padded to its owner's size.
    assert_eq!(plc.read_located("%QB209").expect("read")[0], byte);
}

// ---------------------------------------------------------------------------
// Addresses that share bytes, none holding the others
// ---------------------------------------------------------------------------

/// FUNCTIONs that write addresses which share bytes when the workspace
/// counts them, none holding the others: `%MW7` and `%MW8` share byte 8,
/// `%MW8` and `%MW9` byte 9, and `%MB8` and `%MX8.3` are in byte 8. Each
/// reads back what one image of bytes would hold.
const SHARED: &str = "
FUNCTION first_word : WORD
    %MW7 := WORD#16#1234;
    %MW8 := WORD#16#5678;
    first_word := %MW7;
END_FUNCTION

FUNCTION shared_byte : BYTE
    %MW7 := WORD#16#1234;
    %MW8 := WORD#16#5678;
    shared_byte := %MB8;
END_FUNCTION

FUNCTION shared_bit : BOOL
    %MW8 := WORD#0;
    %MW7 := WORD#16#0800;
    shared_bit := %MX8.3;
END_FUNCTION

FUNCTION chain_first : WORD
    %MW7 := WORD#16#1111;
    %MW9 := WORD#16#2222;
    %MW8 := WORD#16#3344;
    chain_first := %MW7;
END_FUNCTION

FUNCTION chain_last : WORD
    %MW7 := WORD#16#1111;
    %MW9 := WORD#16#2222;
    %MW8 := WORD#16#3344;
    chain_last := %MW9;
END_FUNCTION
";

/// Counting bytes, addresses that share bytes are stored together: each
/// reads what the others wrote of its bytes, in the area's byte order.
/// Counted by width, the same addresses are apart.
#[rstest]
#[case::by_width("", [0x1234, 0, 0, 0x1111, 0x2222])]
#[case::little(BY_BYTE, [0x7834, 0x78, 1, 0x4411, 0x2233])]
#[case::big(BY_BYTE_BIG, [0x1256, 0x56, 0, 0x1133, 0x4422])]
fn addresses_that_share_bytes_are_stored_together(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] values: [i32; 5],
) {
    let (_, wasm) = built(&mut with_db, section, SHARED);
    let run = |name: &str| execute_wasm::<(), i32>(&wasm, name, ());
    let [first, byte, bit, chain_first, chain_last] = values;
    assert_eq!(run("first_word"), first, "%MW7 after %MW8");
    assert_eq!(run("shared_byte"), byte, "%MB8 after %MW8");
    assert_eq!(run("shared_bit"), bit, "%MX8.3 after %MW7");
    assert_eq!(run("chain_first"), chain_first, "%MW7 after %MW8");
    assert_eq!(run("chain_last"), chain_last, "%MW9 after %MW8");
}

/// A FUNCTION writing `%MW7` and then `%MW8`, declared VAR_GLOBALs both.
const BOTH_DECLARED: &str = "
FUNCTION both : WORD
VAR_EXTERNAL first : WORD; second : WORD; END_VAR
    first := WORD#16#1234;
    second := WORD#16#5678;
    both := first;
END_FUNCTION

PROGRAM P
END_PROGRAM

CONFIGURATION C
    VAR_GLOBAL
        first AT %MW7 : WORD;
        second AT %MW8 : WORD;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";

/// The same with `%MW7` bare.
const ONE_DECLARED: &str = "
FUNCTION both : WORD
VAR_EXTERNAL second : WORD; END_VAR
    %MW7 := WORD#16#1234;
    second := WORD#16#5678;
    both := %MW7;
END_FUNCTION

PROGRAM P
END_PROGRAM

CONFIGURATION C
    VAR_GLOBAL
        second AT %MW8 : WORD;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";

/// Declared or bare, `%MW7` and `%MW8` are stored together, whichever of
/// them the byte order puts first in memory: the second write changes the
/// first word's byte 8.
#[rstest]
#[case::little_declared(BY_BYTE, BOTH_DECLARED, 0x7834)]
#[case::little_bare(BY_BYTE, ONE_DECLARED, 0x7834)]
#[case::big_declared(BY_BYTE_BIG, BOTH_DECLARED, 0x1256)]
#[case::big_bare(BY_BYTE_BIG, ONE_DECLARED, 0x1256)]
fn declared_addresses_that_share_bytes_are_stored_together(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] source: &str,
    #[case] value: i32,
) {
    let (_, wasm) = built(&mut with_db, section, source);
    assert_eq!(execute_wasm::<(), i32>(&wasm, "both", ()), value);
}

/// A host finds each address that shares bytes at its own place: its own
/// bytes, one of them in the other's, in the order memory holds them. A
/// part inside one of them is listed in that one, as a part of any address
/// is, and a part inside both in the lower one, its bit counted from there.
#[rstest]
#[case::little(BY_BYTE, 1, 10, 11)]
#[case::big(BY_BYTE_BIG, -1, 2, 3)]
fn the_located_map_places_each_address_that_shares_bytes(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] apart: i64,
    #[case] shift: u16,
    #[case] shared_shift: u16,
) {
    let source = "
PROGRAM P
VAR
    first AT %QW7 : WORD;
    second AT %QW8 : WORD;
    flag AT %QX9.2 : BOOL;
    edge AT %QX8.3 : BOOL;
END_VAR
    first := WORD#1;
END_PROGRAM

CONFIGURATION C
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";
    let (_, wasm) = built(&mut with_db, section, source);
    let map = located_map(&wasm);
    let (first, second) = (entry(&map, "%QW7"), entry(&map, "%QW8"));
    for word in [first, second] {
        assert!(word.part_of.is_none(), "{} has bytes", word.address);
        assert_eq!(word.size, 2, "{} is a word", word.address);
    }
    let offset = |e: &debug_format::LocatedVar| i64::from(e.offset.expect("an offset"));
    assert_eq!(offset(second) - offset(first), apart);
    assert_eq!(i64::from(second.addr) - i64::from(first.addr), apart);
    let flag = entry(&map, "%QX9.2");
    let of = flag.part_of.as_ref().expect("a part");
    assert_eq!((of.owner.as_str(), of.shift), ("%QW8", shift));
    assert_eq!(flag.offset, second.offset, "in the second word");
    let edge = entry(&map, "%QX8.3");
    let of = edge.part_of.as_ref().expect("a part");
    assert_eq!((of.owner.as_str(), of.shift), ("%QW7", shared_shift));
    assert_eq!(edge.offset, first.offset, "in the first word");
}

/// RETAIN on every declaration of a group keeps all of its bytes, in one
/// range: the storage they share, named by the one that starts it, a bare
/// address as itself.
#[rstest]
#[case::little(BY_BYTE, BOTH_DECLARED, "first")]
#[case::big(BY_BYTE_BIG, BOTH_DECLARED, "second")]
#[case::little_bare(BY_BYTE, ONE_DECLARED, "%MW7")]
#[case::big_bare(BY_BYTE_BIG, ONE_DECLARED, "second")]
fn addresses_that_share_bytes_persist_together(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] source: &str,
    #[case] leader: &str,
) {
    let source = source.replace("VAR_GLOBAL\n", "VAR_GLOBAL RETAIN\n");
    let (mir, _) = built(&mut with_db, section, &source);
    let ranges: Vec<(&str, u32)> = mir
        .retain_map
        .ranges
        .iter()
        .map(|r| (r.path.as_str(), r.size))
        .collect();
    assert_eq!(ranges, [(leader, 3)]);
}

/// VAR_CONFIG locates an instance's variable at `%MW8`, which shares byte 8
/// with a VAR_GLOBAL at `%MW7`: the variable points at its own bytes in the
/// storage they share, whichever of the two the byte order puts first.
#[rstest]
#[case::little(BY_BYTE, 0x7834)]
#[case::big(BY_BYTE_BIG, 0x1256)]
fn a_configured_address_that_shares_bytes_is_stored_with_them(
    mut with_db: db::RootDatabase,
    #[case] section: &str,
    #[case] value: u16,
) {
    let source = "
FUNCTION_BLOCK Motor
VAR
    speed AT %M* : WORD;
END_VAR
    speed := WORD#16#5678;
END_FUNCTION_BLOCK

PROGRAM Line
VAR_EXTERNAL other : WORD; END_VAR
VAR
    a : Motor;
END_VAR
    other := WORD#16#1234;
    a();
END_PROGRAM

CONFIGURATION C
    VAR_GLOBAL
        other AT %MW7 : WORD;
    END_VAR
    VAR_CONFIG
        R.L1.a.speed AT %MW8 : WORD;
    END_VAR
    RESOURCE R ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM L1 WITH T : Line;
    END_RESOURCE
END_CONFIGURATION
";
    let (_, wasm) = built(&mut with_db, section, source);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("a scan");
    assert_eq!(plc.read_var(&info, "other"), Some(VarValue::U16(value)));
}
