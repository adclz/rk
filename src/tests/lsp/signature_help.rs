use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use ide_proto::handlers::signature_help::find_signature_help;
use insta::assert_debug_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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
    TASK t1(PRIORITY := 5);
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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
    TASK t1(PRIORITY := 5);
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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
    TASK t1(SINGLE := %IX0.0, INTERVAL := T#20ms, PRIORITY := 3);
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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
    TASK t1(PRIORITY := 5);
END_CONFIGURATION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor on task name (before the parenthesis) — no signature help
    let offset = source.find("TASK t1(").unwrap() + "TASK ".len();
    let help = find_signature_help(&with_db, file, offset);

    assert!(help.is_none());
}
