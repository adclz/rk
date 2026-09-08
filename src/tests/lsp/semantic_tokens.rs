use auto_lsp::core::semantic_tokens_builder::SemanticTokensBuilder;
use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use ide_proto::CLASS;
use ide_proto::FUNCTION;
use ide_proto::INTERFACE;
use ide_proto::SUPPORTED_TYPES;
use ide_proto::handlers::SemanticTokensHandler;
use ide_proto::walk::WalkHir;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::with_db;

#[rstest]
pub fn pou_tokens_in_variable_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb0

END_FUNCTION_BLOCK

CLASS cl0

END_CLASS

FUNCTION_BLOCK fb1
    VAR
        test: fb0; // highlight fb0
        test2: cl0; // highlight cl0
    END_VAR

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // fb0 and cl0 should be highlighted in variable types
    assert_eq!(
        result.data[0].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32
    );
    assert_eq!(
        result.data[1].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32
    );
}

#[rstest]
pub fn pou_tokens_resolved_path(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb0

END_FUNCTION_BLOCK

CLASS cl0

END_CLASS

FUNCTION_BLOCK fb1
    VAR
        test: fb0; // highlight fb0
        test2: cl0; // highlight cl0
    END_VAR


    test; // highlight fb0
    test2;  // highlight cl0

END_FUNCTION_BLOCK"#;

    assert_snapshot!(rendered(&mut with_db, source), @r"
    fb0 function
    cl0 class
    fb1 function
    test variable
    fb0 function
    test2 variable
    cl0 class
    ");
}

#[rstest]
pub fn class_and_fb_tokens_in_extends(mut with_db: RootDatabase) {
    let source = r#"
CLASS cl0

END_CLASS

FUNCTION_BLOCK fb0 EXTENDS cl0 // highlight cl0

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1 EXTENDS fb1 // highlight fb1

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // fb0 and cl0 should be highlighted in variable types
    assert_eq!(
        result.data[0].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32
    );
    assert_eq!(
        result.data[1].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32
    );
}

#[rstest]
pub fn class_and_fb_tokens_in_implements(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE in0 END_INTERFACE
INTERFACE in1 END_INTERFACE
INTERFACE in2 END_INTERFACE
INTERFACE in3 END_INTERFACE
INTERFACE in4 END_INTERFACE
INTERFACE in5 END_INTERFACE

FUNCTION_BLOCK fb0 MPLEMENTS in0, in1, in2 // highlight in0, in1, in2

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1 IMPLEMENTS in3, in4, in5 // highlight in3, in4, in5

END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    assert_eq!(
        result.data[0].token_type,
        SUPPORTED_TYPES
            .iter()
            .position(|x| *x == INTERFACE)
            .unwrap() as u32
    );
    assert_eq!(
        result.data[1].token_type,
        SUPPORTED_TYPES
            .iter()
            .position(|x| *x == INTERFACE)
            .unwrap() as u32
    );
    assert_eq!(
        result.data[2].token_type,
        SUPPORTED_TYPES
            .iter()
            .position(|x| *x == INTERFACE)
            .unwrap() as u32
    );
    assert_eq!(
        result.data[3].token_type,
        SUPPORTED_TYPES
            .iter()
            .position(|x| *x == INTERFACE)
            .unwrap() as u32
    );
    assert_eq!(
        result.data[4].token_type,
        SUPPORTED_TYPES
            .iter()
            .position(|x| *x == INTERFACE)
            .unwrap() as u32
    );
    assert_eq!(
        result.data[5].token_type,
        SUPPORTED_TYPES
            .iter()
            .position(|x| *x == INTERFACE)
            .unwrap() as u32
    );
}

#[rstest]
pub fn namespace_target_token_in_variable_type(mut with_db: RootDatabase) {
    // For `x : System.Controller`, only "Controller" should get the token, not the whole spec
    let source = r#"
NAMESPACE System
    FUNCTION_BLOCK Controller
    END_FUNCTION_BLOCK
END_NAMESPACE

FUNCTION_BLOCK fb1
    VAR
        x : System.Controller;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(rendered(&mut with_db, source), @r"
    Controller function
    fb1 function
    x variable
    Controller function
    ");
}

#[rstest]
pub fn namespace_target_token_in_extends(mut with_db: RootDatabase) {
    // For `EXTENDS System.Base`, only "Base" should get the token
    let source = r#"
NAMESPACE System
    CLASS Base
    END_CLASS
END_NAMESPACE

FUNCTION_BLOCK MyFB EXTENDS System.Base
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // data[0]: PouDecl Base → CLASS
    // data[1]: PouDecl MyFB → FUNCTION
    // data[2]: Spec extends target "Base" → CLASS
    assert_eq!(
        result.data[2].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32
    );
    assert_eq!(
        result.data[2].length,
        "Base".len() as u32,
        "token should span only the target identifier, not the full namespace path"
    );
}

#[rstest]
pub fn comment_bracket_ref_pou(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
END_FUNCTION_BLOCK

(* Uses [MyFB] internally *)
FUNCTION fn1 : INT
END_FUNCTION"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // data[0]: PouDecl MyFB → FUNCTION
    // data[1]: comment bracket ref [MyFB] → FUNCTION
    // data[2]: PouDecl fn1 → FUNCTION
    assert!(result.data.len() >= 3);
    assert_eq!(
        result.data[1].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32,
        "bracket ref [MyFB] in comment should get FUNCTION token"
    );
    assert_eq!(result.data[1].length, "MyFB".len() as u32);
}

#[rstest]
pub fn comment_bracket_ref_class(mut with_db: RootDatabase) {
    let source = r#"
CLASS MyClass
END_CLASS

(* See [MyClass] *)
FUNCTION fn1 : INT
END_FUNCTION"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // data[0]: PouDecl MyClass → CLASS
    // data[1]: comment bracket ref [MyClass] → CLASS
    // data[2]: PouDecl fn1 → FUNCTION
    assert!(result.data.len() >= 3);
    assert_eq!(
        result.data[1].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32,
        "bracket ref [MyClass] in comment should get CLASS token"
    );
}

#[rstest]
pub fn comment_bracket_ref_on_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Actuator
END_FUNCTION_BLOCK

FUNCTION fn1 : INT
VAR
    (* Controls [Actuator] *)
    x : INT;
END_VAR
END_FUNCTION"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // data[0]: PouDecl Actuator → FUNCTION
    // data[1]: PouDecl fn1 → FUNCTION
    // data[2]: comment bracket ref [Actuator] on variable → FUNCTION
    assert!(result.data.len() >= 3);
    assert_eq!(
        result.data[2].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32,
        "bracket ref [Actuator] in variable comment should get FUNCTION token"
    );
    assert_eq!(result.data[2].length, "Actuator".len() as u32);
}

#[rstest]
pub fn comment_bracket_ref_unresolved_no_token(mut with_db: RootDatabase) {
    let source = r#"
(* See [NonExistent] *)
FUNCTION fn1 : INT
END_FUNCTION"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // Only data[0]: PouDecl fn1 → FUNCTION; no token for unresolved [NonExistent]
    assert_eq!(result.data.len(), 1);
    assert_eq!(
        result.data[0].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == FUNCTION).unwrap() as u32,
    );
}

#[rstest]
pub fn comment_bracket_ref_multiple_in_one_comment(mut with_db: RootDatabase) {
    let source = r#"
CLASS Sensor
END_CLASS

INTERFACE IController
END_INTERFACE

(* Combines [Sensor] and [IController] *)
FUNCTION_BLOCK MyFB
END_FUNCTION_BLOCK"#;

    add_sources(&mut with_db, &[source]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new("".into());

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut builder);
        std::ops::ControlFlow::Continue(())
    });

    let result = builder.build();

    // data[0]: PouDecl Sensor → CLASS
    // data[1]: PouDecl IController → INTERFACE
    // data[2]: comment [Sensor] → CLASS
    // data[3]: comment [IController] → INTERFACE
    // data[4]: PouDecl MyFB → FUNCTION
    assert!(result.data.len() >= 5);
    assert_eq!(
        result.data[2].token_type,
        SUPPORTED_TYPES.iter().position(|x| *x == CLASS).unwrap() as u32,
    );
    assert_eq!(result.data[2].length, "Sensor".len() as u32);
    assert_eq!(
        result.data[3].token_type,
        SUPPORTED_TYPES
            .iter()
            .position(|x| *x == INTERFACE)
            .unwrap() as u32,
    );
    assert_eq!(result.data[3].length, "IController".len() as u32);
}

/// What a name IS, not what it is OF. Every variable used to take its
/// type's colour, so an enum-typed one read as the enum; declarations,
/// parameters and fields had no colour at all; and a spec's own span
/// coloured a whole `STRUCT ... END_STRUCT` as one token.
#[rstest]
fn every_name_is_coloured_as_what_it_is(mut with_db: RootDatabase) {
    let source = r#"
TYPE Mode : (Idle, Running); END_TYPE
TYPE Rec : STRUCT a : INT; END_STRUCT END_TYPE

INTERFACE Itf
METHOD Halt : INT
END_METHOD
END_INTERFACE

FUNCTION_BLOCK fb
VAR_INPUT
    p : INT;
END_VAR
VAR
    m : Mode;
    r : Rec;
END_VAR
METHOD Spin : INT
END_METHOD
    m := Mode#Running;
    r.a := p;
END_FUNCTION_BLOCK

PROGRAM prog
VAR
    inst : fb;
END_VAR
    inst(p := 1);
END_PROGRAM
"#;
    assert_snapshot!(rendered(&mut with_db, source), @r"
    Mode enum
    Rec struct
    Itf interface
    Halt method
    fb function
    p parameter
    m variable
    Mode enum
    r variable
    Rec struct
    Spin method
    m variable
    Mode enum
    Running enumMember
    r.a property
    p parameter
    prog function
    inst variable
    fb function
    ");
}

/// Each token as the text it covers and what it was called, which is what a
/// reader is checking. Indexing the raw stream broke whenever a token was
/// added anywhere before the one under test.
fn rendered(db: &mut RootDatabase, source: &str) -> String {
    add_sources(db, &[source]);
    let sema = semantic_index(db, *db.get_files().iter().last().unwrap());
    let mut builder = SemanticTokensBuilder::new(String::new());
    let _ = sema.walk_hir(db, &mut |node| {
        node.semantic_tokens(db, &mut builder);
        std::ops::ControlFlow::<()>::Continue(())
    });

    let lines: Vec<&str> = source.lines().collect();
    let (mut line, mut col) = (0usize, 0usize);
    builder
        .build()
        .data
        .iter()
        .map(|token| {
            line += token.delta_line as usize;
            col = match token.delta_line {
                0 => col + token.delta_start as usize,
                _ => token.delta_start as usize,
            };
            let text = lines
                .get(line)
                .map(|l| {
                    let start = col.min(l.len());
                    &l[start..(col + token.length as usize).min(l.len())]
                })
                .unwrap_or("?");
            format!(
                "{text} {}",
                SUPPORTED_TYPES[token.token_type as usize].as_str()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
