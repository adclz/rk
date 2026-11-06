// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Enum codegen across contexts: a name is the value its declaration gives it,
//! stored at the enum's base type, or as a DINT when it has none, through FB
//! fields, function parameters and results, initializers, CASE labels, arrays
//! and struct fields. A type with named values holds its base's other values too.
//! (Regression: `EnumValue` lowering was a stub emitting 0 for EVERY variant,
//! and `Type::EnumVariant` had no elementary mapping.)

use crate::tests::codegen::with_db;
use rstest::*;

/// A configuration that runs `Main` as `P1`.
const RUNS_MAIN: &str = "
    CONFIGURATION Cfg
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM P1 WITH T : Main;
        END_RESOURCE
    END_CONFIGURATION
";

/// What each of `paths` holds after one scan of `P1`, read from memory through the
/// debug tables: an enum's value as stored, a DINT, a BOOL as 0 or 1. A CASE label
/// and an argument agree under any numbering. Memory shows which numbering was used.
fn stored(db: &mut db::RootDatabase, source: &str, paths: &[&str]) -> Vec<i64> {
    use debug_format::VarValue;
    let (_mir, wasm) =
        crate::tests::codegen::compile_to_mir_and_wasm(db, &format!("{source}{RUNS_MAIN}"));
    let mut plc = crate::tests::codegen::TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let info = debug_format::DebugInfo::from_wasm(&wasm);
    paths
        .iter()
        .map(|path| match plc.read_var(&info, path) {
            Some(VarValue::Enum { raw, .. }) => raw,
            Some(VarValue::I32(n)) => i64::from(n),
            Some(VarValue::Bool(b)) => i64::from(b),
            other => panic!("{path} is {other:?}"),
        })
        .collect()
}

/// Enum VAR_INPUT written at the FB call site, compared against a literal in
/// the body, stored into enum instance state.
#[rstest]
fn fb_enum_input_and_state(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE

        FUNCTION_BLOCK picker
        VAR_INPUT c : Color; END_VAR
        VAR last : Color; END_VAR
        VAR_OUTPUT hit : INT; END_VAR
            IF c = Color#Green THEN
                hit := hit + 1;
            END_IF;
            last := c;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR p : picker; END_VAR
            p(c := Color#Red);
            p(c := Color#Green);
            p(c := Color#Green);
            test := p.hit;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 2, "two Green hits");
}

/// CASE over an enum: each variant label lowers to its own ordinal.
#[rstest]
fn case_over_enum_variants(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE

        FUNCTION pick : INT
        VAR_INPUT c : Color; END_VAR
            CASE c OF
                Color#Red: pick := 10;
                Color#Green: pick := 20;
                Color#Blue: pick := 30;
            END_CASE;
        END_FUNCTION

        FUNCTION test : INT
            test := pick(c := Color#Blue) * 100 + pick(c := Color#Green);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 3020, "Blue -> 30, Green -> 20");
}

/// Function-local enum with an initializer, reassignment, and equality /
/// inequality comparisons.
#[rstest]
fn function_local_enum(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Mode : (Idle, Run, Halt); END_TYPE

        FUNCTION test : INT
        VAR m : Mode := Mode#Run; END_VAR
            test := 0;
            IF m = Mode#Run THEN
                test := test + 1;
            END_IF;
            m := Mode#Halt;
            IF m <> Mode#Run THEN
                test := test + 10;
            END_IF;
            IF m = Mode#Halt THEN
                test := test + 100;
            END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 111, "init honored, reassign + =/<> comparisons");
}

/// Enum as a FUNCTION return value, consumed by the caller's comparison.
#[rstest]
fn enum_function_return(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Mode : (Idle, Run, Halt); END_TYPE

        FUNCTION next : Mode
        VAR_INPUT m : Mode; END_VAR
            IF m = Mode#Idle THEN
                next := Mode#Run;
            ELSE
                next := Mode#Halt;
            END_IF;
        END_FUNCTION

        FUNCTION test : INT
        VAR m : Mode; END_VAR
            m := next(m := Mode#Idle);
            IF m = Mode#Run THEN
                test := 1;
            ELSE
                test := 0;
            END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "Idle -> Run round-trips through the return");
}

/// Arrays of enums: element writes and reads keep distinct ordinals.
#[rstest]
fn array_of_enums(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE

        FUNCTION test : INT
        VAR a : ARRAY[0..2] OF Color; i : INT; hits : INT; END_VAR
            a[0] := Color#Blue;
            a[1] := Color#Green;
            a[2] := Color#Blue;
            FOR i := 0 TO 2 DO
                IF a[i] = Color#Blue THEN
                    hits := hits + 1;
                END_IF;
            END_FOR;
            test := hits;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 2, "two Blue elements");
}

/// An enum field inside a struct: write via the field path, compare back.
#[rstest]
fn enum_struct_field(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green, Blue); END_TYPE
        TYPE Pixel : STRUCT c : Color; brightness : INT; END_STRUCT; END_TYPE

        FUNCTION test : INT
        VAR p : Pixel; END_VAR
            p.c := Color#Green;
            p.brightness := 7;
            IF p.c = Color#Green THEN
                test := p.brightness;
            ELSE
                test := 0;
            END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 7, "enum struct field round-trips");
}

/// IEC 61131-3: an enumerated type may assign explicit values to its
/// enumerators (`(Idle := 10, Run := 20)`), and every later enumerator without
/// one continues from the previous value. MIR used to number variants by
/// position and ignore `EnumVariant.value` entirely, so the declared values
/// were silently replaced by 0, 1, 2 — wrong for any CASE dispatch, INT
/// comparison, or value transmitted to a device. Memory holds the declared values,
/// and a CASE dispatches on them.
#[rstest]
fn explicit_enum_values_are_honored(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Mode : (Idle := 10, Run := 20, Halt := 30); END_TYPE

        FUNCTION code : DINT
        VAR_INPUT m : Mode; END_VAR
            CASE m OF
                Mode#Idle: code := 1;
                Mode#Run:  code := 2;
                Mode#Halt: code := 3;
            ELSE
                code := -1;
            END_CASE;
        END_FUNCTION

        PROGRAM Main
        VAR run, halt : Mode; dispatched : DINT; END_VAR
            run := Mode#Run;
            halt := Mode#Halt;
            dispatched := code(m := run) * 100 + code(m := halt);
        END_PROGRAM
    "#;
    assert_eq!(
        stored(
            &mut with_db,
            source,
            &["P1.run", "P1.halt", "P1.dispatched"]
        ),
        [20, 30, 203],
        "the declared values are stored, not the positions, and CASE dispatches on them"
    );
}

/// Enumerators after an explicit value continue from it (`(A := 5, B, C)` is
/// 5, 6, 7) — the IEC/C-style continuation rule, as memory holds them.
#[rstest]
fn enum_values_continue_after_an_explicit_one(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Step : (A := 5, B, C); END_TYPE

        FUNCTION code : DINT
        VAR_INPUT s : Step; END_VAR
            CASE s OF
                Step#A: code := 1;
                Step#B: code := 2;
                Step#C: code := 3;
            ELSE
                code := -1;
            END_CASE;
        END_FUNCTION

        PROGRAM Main
        VAR a, b, c : Step; dispatched : DINT; END_VAR
            a := Step#A;
            b := Step#B;
            c := Step#C;
            dispatched := code(s := b) * 10 + code(s := c);
        END_PROGRAM
    "#;
    assert_eq!(
        stored(
            &mut with_db,
            source,
            &["P1.a", "P1.b", "P1.c", "P1.dispatched"]
        ),
        [5, 6, 7, 23],
        "B and C continue from A's explicit 5"
    );
}

/// The declared base is the storage lane: an LINT-based enum is an 8-byte
/// field carrying values above 2^31, where the hardcoded DInt storage
/// truncated them and fed an i32 literal lane to an i64 load. (A SINT base
/// still occupies a 4-byte slot — every type of 32 bits or less does, by
/// this compiler's slot layout — so the lane, not the slot, is what the
/// base changes there.) The retain band is the observable: real layout,
/// real bytes.
#[rstest]
fn typed_enum_storage_follows_the_declared_base(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Small : SINT (Lo, Hi); END_TYPE
        TYPE Wide : LINT (Zero, Big := 16#1_0000_0000); END_TYPE

        PROGRAM P
        VAR RETAIN s : Small; w : Wide; END_VAR
            s := Small#Hi;
            w := Wide#Big;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, wasm) = super::compile_to_mir_and_wasm(&mut with_db, source);
    // s: the 4-byte slot at 0 every type of 32 bits or less takes, w: 8 bytes at
    // 8 — 16 in all. The hardcoded-DInt world packed both as 4-byte fields into 8.
    assert_eq!(mir.retain_size, 16, "storage widths must follow the bases");

    let mut plc = crate::tests::codegen::TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let band = plc.read_retain();
    assert_eq!(band[0] as i8, 1, "Small#Hi is one SINT byte");
    assert_eq!(
        i64::from_le_bytes(band[8..16].try_into().unwrap()),
        0x1_0000_0000,
        "Wide#Big keeps its 33-bit value"
    );
}

/// Comparisons and CASE run at the declared lane — a wide enum's equality
/// is an i64 compare fed by an i64 literal, not a truncated i32 one.
#[rstest]
fn wide_enum_compares_and_matches_at_its_lane(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Wide : LINT (Zero, Big := 16#1_0000_0000, Bigger); END_TYPE

        FUNCTION pick : DINT
        VAR_INPUT w : Wide; END_VAR
            CASE w OF
                Wide#Big:    pick := 1;
                Wide#Bigger: pick := 2;
            ELSE
                pick := 0;
            END_CASE;
        END_FUNCTION

        FUNCTION test : DINT
        VAR w : Wide; ok : DINT; END_VAR
            w := Wide#Bigger;
            IF w = Wide#Bigger THEN ok := 100; END_IF;
            test := ok + pick(w := Wide#Big) * 10 + pick(w := w);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 112, "equality 100 + Big 10 + Bigger 2");
}

/// A value that folds is the value: Idle := 1 + 1 is 2, and Run follows at 3, as
/// memory holds them. This shape used to abort lowering after a clean check.
#[rstest]
fn enum_value_folding_expression_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Mode : (Idle := 1 + 1, Run) END_TYPE

        PROGRAM Main
        VAR idle, run : Mode; same : BOOL; END_VAR
            idle := Mode#Idle;
            run := Mode#Run;
            same := run = Mode#Run;
        END_PROGRAM
    "#;
    assert_eq!(
        stored(&mut with_db, source, &["P1.idle", "P1.run", "P1.same"]),
        [2, 3, 1],
        "Run follows the folded 2"
    );
}

/// A variant at the edge of its storage survives a store: `B` is 127, the
/// last SINT. One past it is refused (E0605); it wrapped and read back as
/// another variant.
#[rstest]
fn a_variant_at_the_storage_edge_survives_a_store(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Code : SINT (A := 126, B); END_TYPE
        TYPE Holder : STRUCT c : Code; END_STRUCT END_TYPE

        FUNCTION test : INT
        VAR h : Holder; END_VAR
            h.c := Code#B;
            IF h.c = Code#B THEN
                test := 1;
            END_IF;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "Code#B reads back as Code#B");
}

/// A type with named values holds its base's values: the names do not limit it. Each
/// expression holds, in a function with these variables.
#[rstest]
#[case::the_default_is_the_written_one("m = Light#Green")]
#[case::a_constant("set(m, 27) = 27")]
#[case::a_name_is_its_number("Light#Amber = 2")]
#[case::a_number_is_a_name("2 = Light#Amber")]
#[case::a_calculation_from_a_name("Light#Red + 1 = 2")]
#[case::two_names("Light#Red + Light#Green = 4")]
#[case::two_types("Light#Green + Paint#Blue = 7")]
#[case::multiplied("Light#Green * 2 = 6")]
#[case::modulo("Light#Green MOD 2 = 1")]
#[case::negated("-Light#Amber = -2")]
#[case::ordered("Light#Red < Light#Green AND m > 2")]
#[case::as_a_lint("wide(m) = 3")]
#[case::as_a_lreal("real64(m) = 3.0")]
#[case::as_a_real_argument("real32(m) = 3.0")]
#[case::a_name_as_a_real_argument("real32(Light#Amber) = 2.0")]
#[case::a_small_base("little() = 101")]
#[case::a_subscript("indexed() = 8")]
#[case::a_selector("selected(Light#Amber) = 10 AND selected(44) = 20 AND selected(45) = 30")]
#[case::an_initial_number("q = 55")]
fn a_type_with_named_values_holds_numbers(mut with_db: db::RootDatabase, #[case] holds: &str) {
    let source = format!(
        r#"
        TYPE
            Light : INT (Red := 1, Amber := 2, Green := 3) := Light#Green;
            Paint : INT (Rouge := 1, Blue := 4);
            Small : SINT (Lo := 1, Hi := 2);
        END_TYPE

        FUNCTION set : Light
        VAR_IN_OUT v : Light; END_VAR
        VAR_INPUT n : INT; END_VAR
            v := n;
            set := v;
        END_FUNCTION

        FUNCTION wide : LINT
        VAR_INPUT v : Light; END_VAR
            wide := v;
        END_FUNCTION

        FUNCTION real64 : LREAL
        VAR_INPUT v : Light; END_VAR
            real64 := v;
        END_FUNCTION

        FUNCTION real32 : REAL
        VAR_INPUT v : REAL; END_VAR
            real32 := v;
        END_FUNCTION

        FUNCTION little : INT
        VAR s : Small; END_VAR
            s := 100;
            s := s + 1;
            little := s;
        END_FUNCTION

        FUNCTION indexed : INT
        VAR a : ARRAY[0..4] OF INT; m : Light; END_VAR
            m := 2;
            a[m] := 8;
            indexed := a[2];
        END_FUNCTION

        FUNCTION selected : INT
        VAR_INPUT m : Light; END_VAR
            CASE m OF
                Light#Amber: selected := 10;
                44: selected := 20;
            ELSE
                selected := 30;
            END_CASE;
        END_FUNCTION

        FUNCTION test : BOOL
        VAR
            m : Light;
            q : Light := 55;
        END_VAR
            test := {holds};
        END_FUNCTION
    "#
    );
    let result: i32 = crate::tests::codegen::run(&mut with_db, &source, "test", ());
    assert_eq!(result, 1, "{holds}");
}

/// Two names of one enum compare, with a base or without.
#[rstest]
fn two_names_of_one_enum_compare(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Color : (Red, Green);
            Alias : Color;
            Light : INT (Stop := 1, Go := 2);
        END_TYPE

        FUNCTION test : INT
            IF Color#Red = Color#Red THEN test := test + 1; END_IF;
            IF Color#Red <> Color#Green THEN test := test + 2; END_IF;
            IF Color#Red = Color#Green THEN test := test + 4; END_IF;
            IF Alias#Red = Color#Red THEN test := test + 8; END_IF;
            IF Light#Stop < Light#Go THEN test := test + 16; END_IF;
        END_FUNCTION
    "#;
    let result: i32 = crate::tests::codegen::run(&mut with_db, source, "test", ());
    assert_eq!(result, 27);
}

/// What [`a_value_with_named_values_goes_where_its_base_goes`] calls.
const NAMED_VALUES: &str = r#"
    TYPE
        Light : INT (Red := 1, Amber := 2, Green := 3);
        Paint : INT (Rouge := 1, Blue := 4);
        Small : SINT (Lo := 1, Hi := 2);
        Big : LINT (Far := 5000000000, Near := 1);
        Flags : WORD (FA := 1, FB := 2, FC := 4);
        Masks : WORD (M1 := 8);
        Rate : UINT (Slow := 60000, Fast := 2);
        Named : INT (Dflt := 1, Other := 2) := 7;
        Alias : Light;
        OnAlias : Alias := 5;
        Plain : (One, Two);
        Sub : INT (0..100);
        Holder : STRUCT m : Light := 55; f : Flags; END_STRUCT;
        Rec : STRUCT m : Light; a : ARRAY[0..1] OF Light; END_STRUCT;
    END_TYPE

    FUNCTION_BLOCK Fb
    VAR_INPUT in : Light; END_VAR
    VAR_OUTPUT out : Light; END_VAR
    VAR_IN_OUT io : INT; END_VAR
        out := in + 1;
        io := out;
    END_FUNCTION_BLOCK

    CLASS Cl
        METHOD PUBLIC Me : Light
        VAR_INPUT p : Light; END_VAR
            Me := p + 1;
        END_METHOD
    END_CLASS

    FUNCTION takes_int : INT
    VAR_INPUT p : INT; END_VAR
        takes_int := p;
    END_FUNCTION

    FUNCTION takes_light : Light
    VAR_INPUT p : Light; END_VAR
        takes_light := p + 1;
    END_FUNCTION

    FUNCTION takes_real : REAL
    VAR_INPUT p : REAL; END_VAR
        takes_real := p;
    END_FUNCTION

    FUNCTION out_int
    VAR_INPUT p : INT; END_VAR
    VAR_OUTPUT o : INT; END_VAR
        o := p;
    END_FUNCTION

    FUNCTION out_sint
    VAR_OUTPUT o : SINT; END_VAR
        o := -7;
    END_FUNCTION

    FUNCTION inout_light
    VAR_IN_OUT io : Light; END_VAR
        io := io + 1;
    END_FUNCTION

    FUNCTION out_bool
    VAR_OUTPUT o : BOOL; END_VAR
        o := TRUE;
    END_FUNCTION

    FUNCTION dflt : Light
    VAR_INPUT p : Light := 12; END_VAR
        dflt := p;
    END_FUNCTION

    FUNCTION ret_num : Light
        ret_num := 5;
    END_FUNCTION

    FUNCTION ret_name : INT
        ret_name := Light#Amber;
    END_FUNCTION

    FUNCTION over : INT
    VAR_INPUT p : INT; END_VAR
        over := 1;
    END_FUNCTION

    FUNCTION over : INT
    VAR_INPUT p : Light; END_VAR
        over := 2;
    END_FUNCTION

    FUNCTION over2 : INT
    VAR_INPUT p : INT; END_VAR
        over2 := 1;
    END_FUNCTION

    FUNCTION over2 : INT
    VAR_INPUT p : DINT; END_VAR
        over2 := 2;
    END_FUNCTION

    FUNCTION both : INT
    VAR_INPUT p : Light; END_VAR
        both := 1;
    END_FUNCTION

    FUNCTION both : INT
    VAR_INPUT p : Paint; END_VAR
        both := 2;
    END_FUNCTION

    FUNCTION own_or_real : INT
    VAR_INPUT p : Light; END_VAR
        own_or_real := 1;
    END_FUNCTION

    FUNCTION own_or_real : INT
    VAR_INPUT p : REAL; END_VAR
        own_or_real := 2;
    END_FUNCTION

    FUNCTION colour_only : INT
    VAR_INPUT p : Plain; END_VAR
        colour_only := 1;
    END_FUNCTION

    FUNCTION colour_only : INT
    VAR_INPUT p : INT; END_VAR
        colour_only := 2;
    END_FUNCTION

    FUNCTION sum : INT
    VAR_INPUT v : INT...; END_VAR
        sum := ...v +;
    END_FUNCTION

    FUNCTION pick : INT
    VAR_INPUT k : Light; v : INT...; END_VAR
        pick := ...v[k];
    END_FUNCTION
"#;

/// A value of a type with named values goes where a value of its base goes. Each
/// row runs its steps in a function with these variables, and then holds.
#[rstest]
// An overload of the type comes first, then one of the base, then what the base
// widens to. A number comes to the type last.
#[case::an_overload_of_its_type("", "over(m) = 2")]
#[case::an_overload_of_its_base("", "over(j) = 1")]
#[case::a_name_to_an_overload_of_its_type("", "over(Light#Red) = 2")]
#[case::an_overload_of_its_base_before_a_wider_one("", "over2(m) = 1")]
#[case::a_number_to_an_overload_of_the_base("", "over(5) = 1")]
#[case::between_two_types_its_own("", "both(p) = 2 AND both(m) = 1")]
#[case::a_name_between_two_types_its_own("", "both(Paint#Blue) = 2 AND both(Light#Red) = 1")]
#[case::its_own_before_a_real("", "own_or_real(m) = 1 AND own_or_real(Light#Red) = 1")]
#[case::a_plain_name_its_own("", "colour_only(Plain#One) = 1 AND colour_only(1) = 2")]
// Passed by value, either way.
#[case::to_its_base("", "takes_int(m) = 2")]
#[case::a_name_to_its_base("", "takes_int(Light#Amber) = 2")]
#[case::a_number_to_the_type("", "takes_light(5) = 6")]
#[case::its_base_to_the_type("", "takes_light(j) = 4")]
#[case::to_a_real("", "takes_real(m) = 2.0")]
#[case::in_a_variadic("", "sum(m, 1, Light#Red) = 4")]
#[case::reading_a_pack("m := 1;", "pick(m, 10, 20) = 20")]
#[case::a_default_input("", "dflt() = 12")]
#[case::a_number_returned("", "ret_num() = 5")]
#[case::a_name_returned_as_its_base("", "ret_name() = 2")]
// Outputs, in-outs and references: an in-out or a reference of its own type.
#[case::from_an_output("out_int(5, o => m);", "m = 5")]
#[case::from_a_narrower_output("out_sint(o => m);", "m = -7")]
#[case::an_in_out_of_its_type("inout_light(m);", "m = 3")]
#[case::through_a_reference("r := REF(m); r^ := 40;", "m = 40 AND r^ + 1 = 41")]
#[case::a_block("fb(in := 5, io := i);", "fb.out = 6 AND i = 6")]
#[case::a_blocks_output_as_its_base("fb(in := Light#Red, io := i); d := fb.out;", "d = 2")]
#[case::a_blocks_output_connected("fb(in := 1, io := i, out => j);", "j = 2")]
#[case::a_blocks_input_set("fb.in := 5;", "fb.in = 5")]
#[case::a_method("", "cl.Me(5) = 6 AND cl.Me(p := Light#Red) = 2")]
// Start values and where it is held.
#[case::a_struct_member("", "h.m = 55")]
#[case::a_struct_member_written("h.m := 66;", "h.m = 66")]
#[case::a_struct_initial("", "rc.m = 5 AND rc.a[0] = 6 AND rc.a[1] = 2")]
#[case::a_struct_through_a_reference(
    "ps := REF(rc); ps^.m := 9; i := ps^.m + 1;",
    "rc.m = 9 AND i = 10"
)]
#[case::an_array_initial("arr[2] := 8;", "arr[0] = 3 AND arr[1] = 9 AND arr[2] = 8")]
#[case::a_repeated_array_initial("", "filled[1] = 1 AND filled[2] = 7")]
#[case::a_type_default("", "dn = 7")]
#[case::an_alias_written("al := 8; i := al;", "al = 8 AND i = 8")]
#[case::an_alias_calculated("i := al + 1;", "i = 7")]
#[case::a_name_of_an_alias("", "Alias#Red = 1 AND Alias#Red = Light#Red")]
#[case::a_default_on_an_alias("", "oa = 5")]
// An unwritten start value is the first name.
#[case::starts_at_its_first_name("", "p = 1 AND bg = 5000000000 AND fl = 1")]
#[case::an_element_starts_at_its_first_name("", "filled[3] = 1 AND arr[2] = 1")]
#[case::a_member_starts_at_its_first_name("", "h.f = 1")]
// Widening, each base at its own width and sign.
#[case::from_a_narrower_signed("m := si;", "m = -3")]
#[case::from_a_narrower_unsigned("m := us;", "m = 200")]
#[case::into_wider("d := m; l := m; y := m;", "d = 2 AND l = 2 AND y = 2.0")]
#[case::a_lint_base("bg := 6000000000; l := bg;", "l = 6000000000")]
#[case::a_lint_name("bg := Big#Far + 1;", "bg = 5000000001")]
#[case::a_uint_base("rt := 60001; u := rt;", "u = 60001 AND rt > Rate#Slow")]
#[case::from_a_subrange("su := 5; m := su;", "m = 5")]
#[case::into_a_subrange("su := m;", "su = 2")]
// Bit strings.
#[case::or_of_two_names("fl := Flags#FA OR Flags#FC;", "fl = 5")]
#[case::or_of_two_types("w := Flags#FA OR Masks#M1;", "w = 9")]
#[case::and_with_a_variable("fl := 7; fl := fl AND Flags#FB;", "fl = 2")]
#[case::xor_with_its_base("w := 3; w := w XOR Flags#FA;", "w = 2")]
#[case::not_of_a_name("fl := NOT Flags#FA;", "fl = 16#FFFE")]
#[case::not_of_a_variable("fl := 1; w := NOT fl;", "w = 16#FFFE")]
#[case::and_on_a_number_base("i := m AND 1;", "i = 0")]
// A part of it, as a part of its base.
#[case::a_bit("fl := 4;", "fl.2 AND NOT fl.1")]
#[case::a_bit_of_a_number_base("m := 4;", "m.2")]
#[case::a_bit_written("fl.2 := TRUE;", "fl = 5")]
#[case::a_bit_of_a_number_base_written("m := 0; m.0 := TRUE;", "m = 1")]
#[case::a_byte("fl := 16#0102;", "fl.%B1 = 1 AND fl.%B0 = 2")]
#[case::a_byte_written("fl := 1; fl.%B1 := 1;", "fl = 16#0101")]
#[case::a_bit_of_a_member("h.f.3 := TRUE;", "h.f = 9 AND h.f.3")]
#[case::a_bit_of_an_element("bits[1].3 := TRUE;", "bits[1] = 9 AND bits[1].3")]
#[case::a_bit_through_a_reference("rf := REF(fl); rf^.3 := TRUE;", "fl = 9 AND rf^.3")]
#[case::a_bit_of_a_blocks_member("fb.in.0 := FALSE;", "NOT fb.in.0 AND fb.in = 0")]
#[case::a_bit_from_an_output("fl := 0; out_bool(o => fl.2);", "fl = 4")]
#[case::a_bit_into_a_bit("fl := 4; fl.3 := fl.2;", "fl = 12")]
// Arithmetic, and two types calculated.
#[case::a_power("x := 2.0 ** m;", "x = 4.0")]
#[case::divided("i := m / 2;", "i = 1")]
#[case::with_a_real("x := 1.5; y := m * x;", "y = 3.0")]
#[case::against_a_real("x := 2.5;", "m < x")]
#[case::against_a_wider("d := 2;", "m = d AND d = Light#Amber")]
#[case::calculated_into_another_type("p := m + 1;", "p = 3")]
#[case::a_negated_name_into_another_type("p := -Light#Red;", "p = -1")]
// Control flow.
#[case::a_for_start("j := 0; FOR i := m TO 3 DO j := i; END_FOR;", "j = 3")]
#[case::for_bounds_by_name(
    "j := 0; FOR i := Light#Red TO Light#Green DO j := i; END_FOR;",
    "j = 3"
)]
#[case::a_for_counter("j := 0; FOR m := 1 TO 3 DO j := m; END_FOR;", "j = 3")]
#[case::a_for_counter_stepping(
    "j := 0; FOR p := Paint#Rouge TO Paint#Blue BY 3 DO j := p; END_FOR;",
    "j = 4"
)]
#[case::a_small_for_counter_after_the_loop("FOR s := 1 TO 3 DO j := s; END_FOR;", "s = 4")]
#[case::a_while("WHILE m < 4 DO m := m + 1; END_WHILE;", "m = 4")]
#[case::a_repeat("REPEAT m := m + 1; UNTIL m = 4 END_REPEAT;", "m = 4")]
#[case::a_case_of_numbers("CASE m OF 1: i := 10; 2: i := 20; ELSE i := 30; END_CASE;", "i = 20")]
#[case::a_case_of_names(
    "CASE m OF Light#Red: i := 10; Light#Amber: i := 20; ELSE i := 30; END_CASE;",
    "i = 20"
)]
#[case::a_case_of_a_range("CASE m OF 1..5: i := 10; ELSE i := 30; END_CASE;", "i = 10")]
#[case::a_name_labelling_its_base(
    "CASE j OF Light#Green: i := 10; ELSE i := 30; END_CASE;",
    "i = 10"
)]
#[case::a_subscript("m := 2; list[m] := 8;", "list[2] = 8 AND list[Light#Amber] = 8")]
fn a_value_with_named_values_goes_where_its_base_goes(
    mut with_db: db::RootDatabase,
    #[case] steps: &str,
    #[case] holds: &str,
) {
    let source = format!(
        r#"{NAMED_VALUES}
        FUNCTION test : BOOL
        VAR
            m : Light := 2;
            p : Paint;
            s : Small;
            bg : Big;
            fl : Flags;
            rt : Rate;
            dn : Named;
            al : Alias := 6;
            oa : OnAlias;
            su : Sub;
            h : Holder;
            arr : ARRAY[0..2] OF Light := [Light#Green, 9];
            filled : ARRAY[0..3] OF Light := [2(Light#Red), 7];
            rc : Rec := (m := 5, a := [6, Light#Amber]);
            list : ARRAY[0..4] OF INT;
            bits : ARRAY[0..1] OF Flags;
            r : REF_TO Light;
            rf : REF_TO Flags;
            ps : REF_TO Rec;
            fb : Fb;
            cl : Cl;
            i, j : INT := 3;
            si : SINT := -3;
            d : DINT;
            l : LINT;
            us : USINT := 200;
            u : UINT;
            w : WORD;
            x : REAL;
            y : LREAL;
        END_VAR
            {steps}
            test := {holds};
        END_FUNCTION
    "#
    );
    let result: i32 = crate::tests::codegen::run(&mut with_db, &source, "test", ());
    assert_eq!(result, 1, "{steps} {holds}");
}

/// A global or a constant of a type with named values starts from a name or a number,
/// and calculates.
#[rstest]
fn a_global_with_named_values_starts_from_a_number(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Light : INT (Red := 1, Amber := 2, Green := 3); END_TYPE

        PROGRAM P
        VAR RETAIN
            held : DINT;
        END_VAR
        VAR_EXTERNAL
            global : Light;
        END_VAR
        VAR_EXTERNAL CONSTANT
            named : Light;
            number : Light;
        END_VAR
            global := global + 1;
            IF global = 45 THEN held := held + 1; END_IF;
            IF named = 3 THEN held := held + 2; END_IF;
            IF number = 9 THEN held := held + 4; END_IF;
            IF named + 1 = 4 THEN held := held + 8; END_IF;
        END_PROGRAM

        CONFIGURATION Cfg
            VAR_GLOBAL
                global : Light := 44;
            END_VAR
            VAR_GLOBAL CONSTANT
                named : Light := Light#Green;
                number : Light := 9;
            END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = crate::tests::codegen::TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("scan");
    let held = i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
    assert_eq!(held, 15);
}

/// The standard functions take a value of a type with named values as they take its
/// base, and read a number from it.
#[rstest]
fn the_standard_functions_take_named_values(mut with_db: db::RootDatabase) {
    let holds = [
        "ABS(m) = 2",
        "MIN(m, 5) = 2",
        "MIN(m, n) = 2",
        "MIN(j, m) = 2",
        "MAX(Light#Red, Light#Green) = 3",
        "LIMIT(0, m, 1) = 1",
        "SEL(TRUE, m, 5) = 5",
        "MUX(Light#Red, 10, 20, 30) = 20",
        "MUX(one, 5, 6) = 6",
        "MOVE(m) = 2",
        "INT_TO_REAL(m) = 2.0",
        "INT_TO_STRING(m) = '2'",
        "SHL(Flags#FA, 2) = 4",
        "EXPT(2.0, m) = 4.0",
    ];
    let mut source = String::from(
        r#"
        USING Std.Math;
        USING Std.Selection;
        USING Std.Convert;
        USING Std.Memory;
        USING Std.Bits;

        TYPE
            Light : INT (Red := 1, Amber := 2, Green := 3);
            Flags : WORD (FA := 1, FB := 2, FC := 4);
        END_TYPE

        FUNCTION test : DINT
        VAR
            m, n : Light := 2;
            one : Light := 1;
            j : INT := 3;
        END_VAR
        "#,
    );
    for (i, holds) in holds.iter().enumerate() {
        source += &format!("IF NOT ({holds}) THEN test := {}; RETURN; END_IF;\n", i + 1);
    }
    source += "END_FUNCTION\n";
    let wasm = crate::tests::codegen::compile_with_libraries(
        &mut with_db,
        &[
            include_str!("../../../stdlib/Math.st"),
            include_str!("../../../stdlib/Selection.st"),
            include_str!("../../../stdlib/Convert.st"),
            include_str!("../../../stdlib/Memory.st"),
            include_str!("../../../stdlib/Bits.st"),
            include_str!("../../../stdlib/Strings.st"),
        ],
        &source,
    );
    let failed: i32 = crate::tests::codegen::execute_wasm(&wasm, "test", ());
    if failed != 0 {
        panic!("{} is false", holds[failed as usize - 1]);
    }
}

/// A name of a type with named values is a constant of its base, wherever a
/// constant is needed. Each row is a program whose `test` holds. A name did not
/// fold: these were refused, and a CASE label that is a CONSTANT of such a type
/// stopped the compiler.
#[rstest]
#[case::a_constant(
    "FUNCTION test : BOOL VAR CONSTANT k : INT := Light#Red + 1; END_VAR test := k = 2; END_FUNCTION"
)]
#[case::a_negated_constant(
    "FUNCTION test : BOOL VAR CONSTANT k : INT := -Light#Green; END_VAR test := k = -3; END_FUNCTION"
)]
#[case::a_constant_of_the_type(
    "FUNCTION test : BOOL VAR CONSTANT k : Light := Light#Green * 2; END_VAR test := k = 6; END_FUNCTION"
)]
#[case::through_an_alias(
    "FUNCTION test : BOOL VAR CONSTANT k : INT := Alias#Amber + 1; END_VAR test := k = 3; END_FUNCTION"
)]
#[case::as_the_storage_holds_it(
    "TYPE Small : SINT (Low := 16#FF); END_TYPE
     FUNCTION test : BOOL VAR CONSTANT k : SINT := Small#Low; END_VAR test := k = -1; END_FUNCTION"
)]
#[case::an_array_bound(
    "FUNCTION test : BOOL VAR a : ARRAY[Light#Red..Light#Green] OF INT; END_VAR
         a[3] := 7; test := a[3] = 7;
     END_FUNCTION"
)]
#[case::an_array_type_bound(
    "TYPE Row : ARRAY[Light#Red..Light#Green] OF INT; END_TYPE
     FUNCTION test : BOOL VAR a : Row; END_VAR a[1] := 7; test := a[1] = 7; END_FUNCTION"
)]
#[case::a_bound_from_a_constant_of_the_type(
    "FUNCTION test : BOOL
     VAR CONSTANT k : Light := 2; END_VAR
     VAR a : ARRAY[0..k] OF INT; END_VAR
         a[2] := 7; test := a[2] = 7;
     END_FUNCTION"
)]
#[case::a_bound_from_a_global_constant(
    "CONFIGURATION Cfg VAR_GLOBAL CONSTANT g : INT := Light#Red + 1; END_VAR END_CONFIGURATION
     FUNCTION test : BOOL
     VAR_EXTERNAL CONSTANT g : INT; END_VAR
     VAR a : ARRAY[0..g] OF INT; END_VAR
         a[2] := 7; test := a[2] = 7;
     END_FUNCTION"
)]
#[case::a_strings_length(
    "FUNCTION test : BOOL VAR s : STRING[Light#Green]; long : STRING := 'abcd'; END_VAR
         s := long; test := s = 'abc';
     END_FUNCTION"
)]
#[case::a_subranges_bounds(
    "FUNCTION test : BOOL VAR r : INT (Light#Red..Light#Green); END_VAR r := 2; test := r = 2; END_FUNCTION"
)]
#[case::another_types_values(
    "TYPE Next : INT (First := Light#Green, Second := Light#Green + 1, Third); END_TYPE
     FUNCTION test : BOOL test := Next#First = 3 AND Next#Second = 4 AND Next#Third = 5; END_FUNCTION"
)]
#[case::its_own_values(
    "TYPE Step : INT (One := 1, Two := Step#One + 1); END_TYPE
     FUNCTION test : BOOL test := Step#Two = 2; END_FUNCTION"
)]
#[case::a_chain_of_its_own_values(
    "TYPE Step : INT (One := 1, Two := Step#One + 1, Ten := Step#Two * 5, Eleven); END_TYPE
     FUNCTION test : BOOL VAR a : ARRAY[0..Step#Ten] OF INT; m : Step := Step#Eleven; END_VAR
         a[10] := 7;
         CASE m OF Step#Eleven: test := a[10] = 7 AND Step#Ten = 10; END_CASE;
     END_FUNCTION"
)]
#[case::a_chain_through_another_type(
    "TYPE Base : INT (X := 1, Y := Base#X + 1); Scaled : INT (A := Base#Y * 10, B); END_TYPE
     FUNCTION test : BOOL test := Scaled#A = 20 AND Scaled#B = 21; END_FUNCTION"
)]
#[case::the_standards_colors(
    "TYPE Colors : DWORD (Red := 16#00FF0000, Green := 16#0000FF00, Blue := 16#000000FF,
         White := Colors#Red OR Colors#Green OR Colors#Blue,
         Black := Colors#Red AND Colors#Green AND Colors#Blue) := Colors#Green;
     END_TYPE
     FUNCTION test : BOOL VAR c : Colors; END_VAR
         test := c = Colors#Green AND Colors#White = 16#00FFFFFF AND Colors#Black = 0
             AND Colors#White = (Colors#Red OR Colors#Green OR Colors#Blue);
     END_FUNCTION"
)]
#[case::its_values_by_bits(
    "TYPE Flags : BYTE (A := 1, B := 2, AB := Flags#A OR Flags#B, NotA := NOT Flags#A, Odd := Flags#A XOR 16#FF);
     END_TYPE
     FUNCTION test : BOOL
         test := Flags#AB = 3 AND Flags#NotA = 16#FE AND Flags#Odd = 16#FE AND Flags#NotA = NOT Flags#A;
     END_FUNCTION"
)]
#[case::a_calculated_label(
    "FUNCTION test : BOOL VAR m : Light := 2; END_VAR
         CASE m OF Light#Red + 1: test := TRUE; END_CASE;
     END_FUNCTION"
)]
#[case::a_range_of_labels(
    "FUNCTION test : BOOL VAR m : Light := 2; END_VAR
         CASE m OF Light#Red..Light#Green: test := TRUE; END_CASE;
     END_FUNCTION"
)]
#[case::a_label_from_a_constant_of_the_type(
    "FUNCTION test : BOOL
     VAR CONSTANT k : Light := 2; END_VAR
     VAR m : Light := 2; END_VAR
         CASE m OF k: test := TRUE; END_CASE;
     END_FUNCTION"
)]
#[case::a_step(
    "FUNCTION test : BOOL VAR i, n : INT; END_VAR
         FOR i := 1 TO 9 BY Light#Amber DO n := n + 1; END_FOR;
         test := n = 5;
     END_FUNCTION"
)]
#[case::an_initial_index(
    "FUNCTION test : BOOL VAR a : ARRAY[0..4] OF INT := [[Light#Amber] := 7]; END_VAR test := a[2] = 7; END_FUNCTION"
)]
#[case::an_initial_index_from_a_constant_of_the_type(
    "FUNCTION test : BOOL
     VAR CONSTANT k : Light := 3; END_VAR
     VAR a : ARRAY[0..4] OF INT := [[k] := 7]; END_VAR
         test := a[3] = 7;
     END_FUNCTION"
)]
#[case::initial_values_at_their_types(
    "FUNCTION test : BOOL
     VAR x : REAL := Light#Green; l : LINT := Light#Green; d : DINT := Light#Red + 1; m : Light := Light#Green; END_VAR
         test := x = 3.0 AND l = 3 AND d = 2 AND m = 3;
     END_FUNCTION"
)]
#[case::field_defaults_at_their_types(
    "TYPE Held : STRUCT m : Light := Light#Green; x : REAL := Light#Amber; END_STRUCT; END_TYPE
     FUNCTION test : BOOL VAR h : Held; END_VAR test := h.m = 3 AND h.x = 2.0; END_FUNCTION"
)]
#[case::elements_at_their_types(
    "FUNCTION test : BOOL
     VAR a : ARRAY[0..1] OF REAL := [Light#Red, Light#Green]; b : ARRAY[0..1] OF Light := [Light#Green, 7]; END_VAR
         test := a[0] = 1.0 AND a[1] = 3.0 AND b[0] = 3 AND b[1] = 7;
     END_FUNCTION"
)]
#[case::a_plain_enum_initial(
    "TYPE Color : (Red, Green); END_TYPE
     FUNCTION test : BOOL VAR c : Color := Color#Green; END_VAR test := c = Color#Green; END_FUNCTION"
)]
fn a_named_value_is_a_constant(mut with_db: db::RootDatabase, #[case] program: &str) {
    let source = format!(
        "TYPE Light : INT (Red := 1, Amber := 2, Green := 3); Alias : Light; END_TYPE\n{program}"
    );
    let result: i32 = crate::tests::codegen::run(&mut with_db, &source, "test", ());
    assert_eq!(result, 1, "{program}");
}

/// A named value folds once however often it is named. Each of these names the
/// two before it: folded again at each name, 22 of them took 10 s to check, and
/// these 60 would not have finished.
#[test]
fn a_named_value_named_twice_folds_once() {
    let chain: String = (2..=60)
        .map(|k| format!(", F{k} := Fib#F{} + Fib#F{}", k - 1, k - 2))
        .collect();
    let source = format!(
        "TYPE Fib : LINT (F0 := 1, F1 := 2{chain}); END_TYPE
         FUNCTION test : BOOL VAR a : ARRAY[0..(Fib#F60 MOD 1000)] OF LINT; END_VAR
             a[881] := Fib#F60;
             test := a[Fib#F60 MOD 1000] = 4052739537881;
         END_FUNCTION"
    );
    let result: i32 = crate::tests::utils::within(std::time::Duration::from_secs(30), move || {
        crate::tests::codegen::run(&mut db::RootDatabase::default(), &source, "test", ())
    });
    assert_eq!(result, 1);
}
