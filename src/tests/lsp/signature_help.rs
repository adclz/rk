use auto_lsp::lsp_types;
use db::RootDatabase;
use ide_proto::handlers::signature_help::find_signature_help;
use insta::assert_debug_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_source, with_db};

#[rstest]
fn signature_help_function_call(mut with_db: RootDatabase) {
    let source = r#"FUNCTION my_func
VAR_INPUT
    a : INT;
    b : BOOL;
END_VAR
END_FUNCTION

FUNCTION caller
VAR
    x : INT;
END_VAR
    my_func(x);
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor right after `(`
    let offset = source.find("my_func(x)").unwrap() + "my_func(".len();
    let help = find_signature_help(&with_db, file, offset);

    assert_debug_snapshot!(help, @r#"
    Some(
        SignatureHelp {
            signatures: [
                SignatureInformation {
                    label: "my_func(a := INT, b := BOOL)",
                    documentation: None,
                    parameters: Some(
                        [
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        8,
                                        16,
                                    ],
                                ),
                                documentation: None,
                            },
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        18,
                                        27,
                                    ],
                                ),
                                documentation: None,
                            },
                        ],
                    ),
                    active_parameter: Some(
                        0,
                    ),
                },
            ],
            active_signature: Some(
                0,
            ),
            active_parameter: Some(
                0,
            ),
        },
    )
    "#);
}

#[rstest]
fn signature_help_active_param(mut with_db: RootDatabase) {
    let source = r#"FUNCTION my_func
VAR_INPUT
    a : INT;
    b : BOOL;
END_VAR
END_FUNCTION

FUNCTION caller
VAR
    x : INT;
    y : BOOL;
END_VAR
    my_func(x, y);
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor on second argument `y`
    let offset = source.find("my_func(x, y)").unwrap() + "my_func(x, ".len();
    let help = find_signature_help(&with_db, file, offset);

    assert_debug_snapshot!(help, @r#"
    Some(
        SignatureHelp {
            signatures: [
                SignatureInformation {
                    label: "my_func(a := INT, b := BOOL)",
                    documentation: None,
                    parameters: Some(
                        [
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        8,
                                        16,
                                    ],
                                ),
                                documentation: None,
                            },
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        18,
                                        27,
                                    ],
                                ),
                                documentation: None,
                            },
                        ],
                    ),
                    active_parameter: Some(
                        1,
                    ),
                },
            ],
            active_signature: Some(
                0,
            ),
            active_parameter: Some(
                1,
            ),
        },
    )
    "#);
}

#[rstest]
fn signature_help_formal_params(mut with_db: RootDatabase) {
    let source = r#"FUNCTION my_func
VAR_INPUT
    a : INT;
    b : BOOL;
END_VAR
END_FUNCTION

FUNCTION caller
VAR
    x : INT;
END_VAR
    my_func(a := x, b := TRUE);
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor on second formal param
    let offset = source.find("b := TRUE").unwrap();
    let help = find_signature_help(&with_db, file, offset);

    assert_debug_snapshot!(help, @r#"
    Some(
        SignatureHelp {
            signatures: [
                SignatureInformation {
                    label: "my_func(a := INT, b := BOOL)",
                    documentation: None,
                    parameters: Some(
                        [
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        8,
                                        16,
                                    ],
                                ),
                                documentation: None,
                            },
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        18,
                                        27,
                                    ],
                                ),
                                documentation: None,
                            },
                        ],
                    ),
                    active_parameter: Some(
                        1,
                    ),
                },
            ],
            active_signature: Some(
                0,
            ),
            active_parameter: Some(
                1,
            ),
        },
    )
    "#);
}

#[rstest]
fn signature_help_with_return_type(mut with_db: RootDatabase) {
    let source = r#"FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
END_FUNCTION

FUNCTION caller
VAR
    result : INT;
END_VAR
    result := add(1, 2);
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("add(1, 2)").unwrap() + "add(".len();
    let help = find_signature_help(&with_db, file, offset);

    assert_debug_snapshot!(help, @r#"
    Some(
        SignatureHelp {
            signatures: [
                SignatureInformation {
                    label: "add(a := INT, b := INT) : INT",
                    documentation: None,
                    parameters: Some(
                        [
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        4,
                                        12,
                                    ],
                                ),
                                documentation: None,
                            },
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        14,
                                        22,
                                    ],
                                ),
                                documentation: None,
                            },
                        ],
                    ),
                    active_parameter: Some(
                        0,
                    ),
                },
            ],
            active_signature: Some(
                0,
            ),
            active_parameter: Some(
                0,
            ),
        },
    )
    "#);
}

#[rstest]
fn signature_help_with_output_param(mut with_db: RootDatabase) {
    let source = r#"FUNCTION my_func
VAR_INPUT
    a : INT;
END_VAR
VAR_OUTPUT
    result : INT;
END_VAR
END_FUNCTION

FUNCTION caller
VAR
    x : INT;
    r : INT;
END_VAR
    my_func(a := x, result => r);
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("a := x").unwrap();
    let help = find_signature_help(&with_db, file, offset);

    assert_debug_snapshot!(help, @r#"
    Some(
        SignatureHelp {
            signatures: [
                SignatureInformation {
                    label: "my_func(a := INT, result => INT)",
                    documentation: None,
                    parameters: Some(
                        [
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        8,
                                        16,
                                    ],
                                ),
                                documentation: None,
                            },
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        18,
                                        31,
                                    ],
                                ),
                                documentation: None,
                            },
                        ],
                    ),
                    active_parameter: Some(
                        0,
                    ),
                },
            ],
            active_signature: Some(
                0,
            ),
            active_parameter: Some(
                0,
            ),
        },
    )
    "#);
}

#[rstest]
fn signature_help_method_call(mut with_db: RootDatabase) {
    let source = r#"FUNCTION_BLOCK MyFB
    VAR
        value : INT;
    END_VAR

    METHOD set_value
    VAR_INPUT
        v : INT;
    END_VAR
        value := v;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION caller
VAR
    fb : MyFB;
END_VAR
    fb.set_value(42);
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("fb.set_value(42)").unwrap() + "fb.set_value(".len();
    let help = find_signature_help(&with_db, file, offset);

    assert_debug_snapshot!(help, @r#"
    Some(
        SignatureHelp {
            signatures: [
                SignatureInformation {
                    label: "set_value(v := INT)",
                    documentation: None,
                    parameters: Some(
                        [
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        10,
                                        18,
                                    ],
                                ),
                                documentation: None,
                            },
                        ],
                    ),
                    active_parameter: Some(
                        0,
                    ),
                },
            ],
            active_signature: Some(
                0,
            ),
            active_parameter: Some(
                0,
            ),
        },
    )
    "#);
}

#[rstest]
fn signature_help_task_config_at_start(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 5);
    END_RESOURCE
END_CONFIGURATION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor right after `(`
    let offset = source.find("TASK t1(").unwrap() + "TASK t1(".len();
    let help = find_signature_help(&with_db, file, offset);

    assert_debug_snapshot!(help, @r#"
    Some(
        SignatureHelp {
            signatures: [
                SignatureInformation {
                    label: "TASK(SINGLE := DataSource, INTERVAL := DataSource, PRIORITY := UINT)",
                    documentation: None,
                    parameters: Some(
                        [
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        5,
                                        25,
                                    ],
                                ),
                                documentation: None,
                            },
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        27,
                                        49,
                                    ],
                                ),
                                documentation: None,
                            },
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        51,
                                        67,
                                    ],
                                ),
                                documentation: None,
                            },
                        ],
                    ),
                    active_parameter: None,
                },
            ],
            active_signature: Some(
                0,
            ),
            active_parameter: None,
        },
    )
    "#);
}

#[rstest]
fn signature_help_task_config_on_priority(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 5);
    END_RESOURCE
END_CONFIGURATION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor on PRIORITY value
    let offset = source.find("PRIORITY := 5").unwrap() + "PRIORITY := ".len();
    let help = find_signature_help(&with_db, file, offset);

    assert!(help.is_some());
    assert_eq!(help.unwrap().active_parameter, None);
}

#[rstest]
fn signature_help_task_config_full(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(SINGLE := %IX0.0, INTERVAL := T#20ms, PRIORITY := 3);
    END_RESOURCE
END_CONFIGURATION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor on INTERVAL value
    let offset = source.find("INTERVAL := T").unwrap() + "INTERVAL := ".len();
    let help = find_signature_help(&with_db, file, offset);

    assert!(help.is_some());
    assert_eq!(help.unwrap().active_parameter, None);
}

#[rstest]
fn signature_help_task_config_before_paren(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 5);
    END_RESOURCE
END_CONFIGURATION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor on task name (before the parenthesis) — no signature help
    let offset = source.find("TASK t1(").unwrap() + "TASK ".len();
    let help = find_signature_help(&with_db, file, offset);

    assert!(help.is_none());
}

// Signature help renders the flattened EXTENDS view for a derived FB call —
// the same list the resolver binds, inherited VAR_IN_OUT included.
#[rstest]
fn signature_help_derived_fb_shows_inherited_params(mut with_db: RootDatabase) {
    let source = r#"FUNCTION_BLOCK base_io
VAR_IN_OUT
    io : INT;
END_VAR
VAR_INPUT
    inp : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived_io EXTENDS base_io
VAR_INPUT
    own : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION caller
VAR
    d : derived_io;
    x : INT;
END_VAR
    d(io := x, inp := 1, own := 2);
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    // Cursor in the third argument: active_parameter must index into the
    // flattened list, not past a 1-entry own-only one.
    let offset = source.find("own := 2").unwrap();
    let help = find_signature_help(&with_db, file, offset);

    assert_debug_snapshot!(help, @r#"
    Some(
        SignatureHelp {
            signatures: [
                SignatureInformation {
                    label: "derived_io(io := INT, inp := INT, own := INT)",
                    documentation: None,
                    parameters: Some(
                        [
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        11,
                                        20,
                                    ],
                                ),
                                documentation: None,
                            },
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        22,
                                        32,
                                    ],
                                ),
                                documentation: None,
                            },
                            ParameterInformation {
                                label: LabelOffsets(
                                    [
                                        34,
                                        44,
                                    ],
                                ),
                                documentation: None,
                            },
                        ],
                    ),
                    active_parameter: Some(
                        2,
                    ),
                },
            ],
            active_signature: Some(
                0,
            ),
            active_parameter: Some(
                2,
            ),
        },
    )
    "#);
}

/// An overloaded name has several declarations behind it. Signature help
/// lists them all and marks the one the call actually resolved to, so the
/// editor can cycle through the rest.
#[rstest]
#[case::the_int_overload("    r := f(1|);", 0, 0)]
#[case::the_real_overload("    x := f(1.0, 2.0|);", 1, 1)]
#[case::still_on_its_first_argument("    x := f(1.0|, 2.0);", 1, 0)]
#[case::nothing_written_yet("    r := f(|);", 0, 0)]
fn an_overloaded_name_lists_every_candidate(
    mut with_db: RootDatabase,
    #[case] call: &str,
    #[case] active_signature: u32,
    #[case] active_parameter: u32,
) {
    let marked = format!(
        r#"FUNCTION f : INT
VAR_INPUT
    a : INT;
END_VAR
END_FUNCTION

FUNCTION f : REAL
VAR_INPUT
    a : REAL;
    b : REAL;
END_VAR
END_FUNCTION

PROGRAM p
VAR
    r : INT;
    x : REAL;
END_VAR
{call}
END_PROGRAM
"#
    );
    let help = help_at(&mut with_db, &marked).expect("signature help");
    let labels: Vec<&str> = help.signatures.iter().map(|s| s.label.as_str()).collect();

    assert_eq!(
        labels,
        ["f(a := INT) : INT", "f(a := REAL, b := REAL) : REAL"]
    );
    assert_eq!(help.active_signature, Some(active_signature));
    assert_eq!(help.active_parameter, Some(active_parameter));
}

/// A PROGRAM and a METHOD hold statements of their own. Neither was searched,
/// so a call in either got no help at all.
#[rstest]
#[case::in_a_program(
    r#"FUNCTION g : INT
VAR_INPUT
    a : INT;
END_VAR
END_FUNCTION

PROGRAM p
VAR
    r : INT;
END_VAR
    r := g(|);
END_PROGRAM
"#
)]
#[case::in_a_method(
    r#"FUNCTION g : INT
VAR_INPUT
    a : INT;
END_VAR
END_FUNCTION

FUNCTION_BLOCK fb
METHOD m : INT
    m := g(|);
END_METHOD
END_FUNCTION_BLOCK
"#
)]
fn a_call_gets_help_wherever_statements_stand(mut with_db: RootDatabase, #[case] marked: &str) {
    let help = help_at(&mut with_db, marked).expect("signature help");

    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.signatures[0].label, "g(a := INT) : INT");
}

/// Asks for help at the `|` marker, which is stripped from the source.
fn help_at(db: &mut RootDatabase, marked: &str) -> Option<lsp_types::SignatureHelp> {
    let (file, offset) = crate::tests::utils::add_marked_source(db, marked);
    find_signature_help(db, file, offset)
}

/// The hint follows the argument being written. It used to be derived from
/// the argument nodes, which do not exist yet for what is half typed, so it
/// jumped ahead on `f(a |)` and stayed behind on `f(a := 1, |)`.
#[rstest]
#[case::empty("f(|)", 0)]
#[case::naming_the_first("f(a|)", 0)]
#[case::after_its_name("f(a |)", 0)]
#[case::mid_assignment("f(a :=|)", 0)]
#[case::its_value("f(a := 1|)", 0)]
#[case::on_the_separator("f(a := 1,|)", 1)]
#[case::after_the_separator("f(a := 1, |)", 1)]
#[case::naming_the_second("f(a := 1, b|)", 1)]
#[case::its_value_too("f(a := 1, b := TRUE|)", 1)]
#[case::positional("f(1, |)", 1)]
#[case::nested_call_is_not_a_separator("f(a := MAX(1, 2), |)", 1)]
#[case::a_comma_in_a_string_is_not_one("f(a := 1, b := 'x, y'|)", 1)]
fn the_active_parameter_follows_the_cursor(
    mut with_db: RootDatabase,
    #[case] call: &str,
    #[case] active_parameter: u32,
) {
    let marked = format!(
        r#"FUNCTION MAX : INT
VAR_INPUT
    x : INT;
    y : INT;
END_VAR
END_FUNCTION

FUNCTION f : INT
VAR_INPUT
    a : INT;
    b : BOOL;
END_VAR
END_FUNCTION

PROGRAM p
VAR
    r : INT;
END_VAR
    r := {call};
END_PROGRAM
"#
    );
    let help = help_at(&mut with_db, &marked).expect("signature help");

    assert_eq!(help.active_parameter, Some(active_parameter));
}
