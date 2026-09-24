use auto_lsp::core::semantic_tokens_builder::SemanticTokensBuilder;
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

use crate::tests::utils::add_source;
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

    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let mut sink = ide_proto::handlers::semantic_tokens::TokenSink::default();

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut sink);
        std::ops::ControlFlow::Continue(())
    });

    let mut builder = SemanticTokensBuilder::new("".into());
    sink.drain_into(&mut builder);
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
    test variable
    test2 variable
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

    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let mut sink = ide_proto::handlers::semantic_tokens::TokenSink::default();

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut sink);
        std::ops::ControlFlow::Continue(())
    });

    let mut builder = SemanticTokensBuilder::new("".into());
    sink.drain_into(&mut builder);
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

    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let mut sink = ide_proto::handlers::semantic_tokens::TokenSink::default();

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut sink);
        std::ops::ControlFlow::Continue(())
    });

    let mut builder = SemanticTokensBuilder::new("".into());
    sink.drain_into(&mut builder);
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
    System namespace
    Controller function
    fb1 function
    x variable
    Controller function
    ");
}

#[rstest]
pub fn namespace_target_token_in_extends(mut with_db: RootDatabase) {
    // For `EXTENDS System.Base`, only "Base" carries the CLASS token: the
    // rendered text IS the source each token covers, so a token over the
    // whole `System.Base` would read `System.Base class` here.
    let source = r#"
NAMESPACE System
    CLASS Base
    END_CLASS
END_NAMESPACE

FUNCTION_BLOCK MyFB EXTENDS System.Base
END_FUNCTION_BLOCK"#;

    assert_snapshot!(rendered(&mut with_db, source), @r"
    System namespace
    Base class
    MyFB function
    Base class
    ");
}

#[rstest]
pub fn comment_bracket_ref_pou(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
END_FUNCTION_BLOCK

(* Uses [MyFB] internally *)
FUNCTION fn1 : INT
END_FUNCTION"#;

    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let mut sink = ide_proto::handlers::semantic_tokens::TokenSink::default();

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut sink);
        std::ops::ControlFlow::Continue(())
    });

    let mut builder = SemanticTokensBuilder::new("".into());
    sink.drain_into(&mut builder);
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

    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let mut sink = ide_proto::handlers::semantic_tokens::TokenSink::default();

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut sink);
        std::ops::ControlFlow::Continue(())
    });

    let mut builder = SemanticTokensBuilder::new("".into());
    sink.drain_into(&mut builder);
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

    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let mut sink = ide_proto::handlers::semantic_tokens::TokenSink::default();

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut sink);
        std::ops::ControlFlow::Continue(())
    });

    let mut builder = SemanticTokensBuilder::new("".into());
    sink.drain_into(&mut builder);
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

    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let mut sink = ide_proto::handlers::semantic_tokens::TokenSink::default();

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut sink);
        std::ops::ControlFlow::Continue(())
    });

    let mut builder = SemanticTokensBuilder::new("".into());
    sink.drain_into(&mut builder);
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

    let file = add_source(&mut with_db, source);

    let sema = semantic_index(&with_db, file);
    let mut sink = ide_proto::handlers::semantic_tokens::TokenSink::default();

    let _ = sema.walk_hir(&with_db, &mut |node| {
        node.semantic_tokens(&with_db, &mut sink);
        std::ops::ControlFlow::Continue(())
    });

    let mut builder = SemanticTokensBuilder::new("".into());
    sink.drain_into(&mut builder);
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
    Idle enumMember
    Running enumMember
    Rec struct
    a property
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
    r variable
    a property
    p parameter
    prog function
    inst variable
    fb function
    inst variable
    p parameter
    ");
}

/// Each token as the text it covers and what it was called, which is what a
/// reader is checking. Indexing the raw stream broke whenever a token was
/// added anywhere before the one under test.
fn rendered(db: &mut RootDatabase, source: &str) -> String {
    let file = add_source(db, source);
    let sema = semantic_index(db, file);
    let mut sink = ide_proto::handlers::semantic_tokens::TokenSink::default();
    let _ = sema.walk_hir(db, &mut |node| {
        node.semantic_tokens(db, &mut sink);
        std::ops::ControlFlow::<()>::Continue(())
    });
    let mut builder = SemanticTokensBuilder::new(String::new());
    sink.drain_into(&mut builder);

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

/// Every name a body writes carries a token, and it is the token for what the
/// name IS at that place. Written as one source so a colour that goes missing
/// shows up as a line that vanished, not as a shifted index.
///
/// Before this: a call site had no token at all (a callee is re-typed as its
/// CALLABLE, which `normalize` peels to the return type), a path was one token
/// over the whole `a.b.c`, a named argument had none, and neither an enum
/// variant nor a struct field carried one where it was DECLARED.
#[rstest]
fn a_body_colours_every_name_it_writes(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE App
    TYPE Mode : (Idle, Running); END_TYPE
    TYPE Point : STRUCT x : INT; END_STRUCT END_TYPE
    TYPE Small : INT (0..10); END_TYPE

    FUNCTION_BLOCK Base
    VAR_INPUT cmd : INT; END_VAR
    VAR_OUTPUT done : INT; END_VAR
        done := cmd;
    END_FUNCTION_BLOCK

    CLASS Motor
    VAR sm : Small; END_VAR
        METHOD PUBLIC Spin : INT
        VAR_INPUT rpm : INT; END_VAR
            Spin := rpm;
        END_METHOD
    END_CLASS

    FUNCTION helper : INT
    VAR_INPUT a : INT; END_VAR
        helper := a;
    END_FUNCTION

    FUNCTION user : INT
    VAR
        mot : Motor;
        b : Base;
        n : INT;
        pt : Point;
        md : Mode;
        ar : ARRAY[1..4] OF INT;
    END_VAR
        helper(1);
        n := helper(a := 2);
        n := mot.Spin(rpm := 3);
        b(cmd := 1, done => n);
        pt.x := 5;
        ar[2] := 6;
        md := Mode#Running;
        n := App.helper(4);
        user := n;
    END_FUNCTION
END_NAMESPACE"#;

    assert_snapshot!(rendered(&mut with_db, source), @r"
    App namespace
    Mode enum
    Idle enumMember
    Running enumMember
    Point struct
    x property
    Small type
    Base function
    cmd parameter
    done parameter
    done parameter
    cmd parameter
    Motor class
    sm variable
    Small type
    Spin method
    rpm parameter
    Spin method
    rpm parameter
    helper function
    a parameter
    helper function
    a parameter
    user function
    mot variable
    Motor class
    b variable
    Base function
    n variable
    pt variable
    Point struct
    md variable
    Mode enum
    ar variable
    helper function
    n variable
    helper function
    a parameter
    n variable
    mot variable
    Spin method
    rpm parameter
    b variable
    cmd parameter
    done parameter
    n variable
    pt variable
    x property
    ar variable
    md variable
    Mode enum
    Running enumMember
    n variable
    App namespace
    helper function
    user function
    n variable
    ");
}

/// A configuration colours the names it writes as what they are: the
/// program's inputs and outputs, the globals they are connected to, the
/// function block a task runs, and each member of a VAR_CONFIG path.
#[rstest]
fn a_configuration_colours_the_names_it_writes(mut with_db: RootDatabase) {
    let tokens = rendered(&mut with_db, crate::tests::lsp::CONNECTED);
    // From the first global on.
    let config = &tokens[tokens.find("w variable").unwrap()..];
    assert_snapshot!(config, @r"
    w variable
    total variable
    d variable
    out variable
    F function
    x1 parameter
    x2 parameter
    w variable
    y1 parameter
    total variable
    fb1 variable
    ");
}
