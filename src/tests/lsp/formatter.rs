use crate::tests::utils::{add_sources, with_db};
use auto_lsp::core::document::Document;
use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use formatter::TOPIARY_LANG;
use insta::assert_snapshot;
use rstest::rstest;
use topiary_core::{Operation, formatter};

pub fn fmt(document: &Document) -> String {
    let mut output = vec![];
    formatter(
        &mut document.texter.text.as_bytes(),
        &mut output,
        &TOPIARY_LANG,
        Operation::Format {
            skip_idempotence: true,
            tolerate_parsing_errors: false,
        },
    )
    .unwrap();

    String::from_utf8(output).unwrap()
}

#[rstest]
pub fn declaration_of_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK myFB

VAR_INPUT IN: BOOL  T1: TIME END_VAR

VAR_OUTPUT OUT: BOOL; ET_OFF: TIME; END_VAR

VAR_IN_OUT A: INT; END_VAR

VAR_TEMP I: INT; END_VAR

VAR B: REAL; END_VAR

VAR_EXTERNAL B: REAL; END_VAR

VAR_EXTERNAL CONSTANT B: REAL; END_VAR
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION_BLOCK myFB

    	VAR_INPUT
    		IN: BOOL;
    		T1: TIME;
    	END_VAR

    	VAR_OUTPUT
    		OUT: BOOL;
    		ET_OFF: TIME;
    	END_VAR

    	VAR_IN_OUT
    		A: INT;
    	END_VAR

    	VAR_TEMP
    		I: INT;
    	END_VAR

    	VAR
    		B: REAL;
    	END_VAR

    	VAR_EXTERNAL
    		B: REAL;
    	END_VAR

    	VAR_EXTERNAL
    		CONSTANT
    		B: REAL;
    	END_VAR
    END_FUNCTION_BLOCK
    ");
}

#[rstest]
pub fn class_definition(mut with_db: RootDatabase) {
    let source = r#"
        CLASS CCounter
	VAR
		m_iCurrentValue: INT; (* Default = 0 *) m_bCountUp: BOOL:=TRUE
	END_VAR

	VAR PUBLIC m_iUpperLimit: INT:=+10000
		        m_iLowerLimit: INT:=-10000
	END_VAR

	METHOD Count (* Only body *)
		IF (m_bCountUp AND m_iCurrentValue<m_iUpperLimit) THEN
	m_iCurrentValue:= m_iCurrentValue+1
		END_IF

		IF (NOT m_bCountUp AND m_iCurrentValue>m_iLowerLimit) THEN
			        m_iCurrentValue:= m_iCurrentValue-1
		END_IF
	END_METHOD

	METHOD SetDirection

            VAR_INPUT
		bCountUp: BOOL;
	END_VAR

            m_bCountUp:=bCountUp;

	END_METHOD
        END_CLASS
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    CLASS CCounter
    	VAR
    		m_iCurrentValue: INT; (* Default = 0 *)
    		m_bCountUp: BOOL := TRUE;
    	END_VAR

    	VAR
    		PUBLIC
    		m_iUpperLimit: INT := + 10000;
    		m_iLowerLimit: INT := - 10000;
    	END_VAR

    	METHOD Count (* Only body *)
    		IF (m_bCountUp AND m_iCurrentValue < m_iUpperLimit) THEN
    			m_iCurrentValue := m_iCurrentValue + 1;
    		END_IF;
    		IF (NOT m_bCountUp AND m_iCurrentValue > m_iLowerLimit) THEN
    			m_iCurrentValue := m_iCurrentValue - 1;
    		END_IF;
    	END_METHOD

    	METHOD SetDirection

    		VAR_INPUT
    			bCountUp: BOOL;
    		END_VAR

    		m_bCountUp := bCountUp;

    	END_METHOD
    END_CLASS
    ");
}

#[rstest]
pub fn case_statement(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	TW:= WORD_BCD_TO_INT(THUMBWHEEL)

	TW_ERROR:= 0

CASE TW OF 1,5: DISPLAY:= OVEN_TEMP
2: DISPLAY:= MOTOR_SPEED
3: DISPLAY:= GROSS - TARE
4,6..10: DISPLAY:= STATUS(TW - 4)
ELSE DISPLAY := 0
TW_ERROR:= 1
END_CASE
QW100:= INT_TO_BCD(DISPLAY)
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn
    	TW := WORD_BCD_TO_INT(THUMBWHEEL);
    	TW_ERROR := 0;
    	CASE TW OF
    		1, 5: DISPLAY := OVEN_TEMP;
    		2: DISPLAY := MOTOR_SPEED;
    		3: DISPLAY := GROSS - TARE;
    		4, 6..10: DISPLAY := STATUS(TW - 4);
    		ELSE
    			DISPLAY := 0;
    			TW_ERROR := 1;
    		END_CASE ;
    		QW100 := INT_TO_BCD(DISPLAY);
    	END_FUNCTION
    ");
}

#[rstest]
pub fn while_statement(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
J:= 1;
WHILE J <= 100 DO
J:= J+2;
END_WHILE
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn
    	J := 1;
    	WHILE J <= 100 DO
    		J := J + 2;
    	END_WHILE;
    END_FUNCTION
    ");
}

#[rstest]
pub fn for_statement(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
J:= 101;
FOR I:= 1 TO 100 BY 2 DO
IF WORDS[I] = 'KEY' THEN
J:= I;
EXIT;
END_IF;
END_FOR;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn
    	J := 101;
    	FOR I := 1 TO 100 BY 2 DO
    		IF WORDS[I] = '' THEN
    			J := I;
    			EXIT;
    		END_IF;
    	END_FOR;
    END_FUNCTION
    ");
}

#[rstest]
pub fn repeat_statement(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
J:= -1;
REPEAT
J:= J+2;
UNTIL J = 101 OR WORDS[J] = 'KEY'
END_REPEAT;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn
    	J := - 1;
    	REPEAT J := J + 2;
    		UNTIL J = 101 OR WORDS[J] = ''
    	END_REPEAT;
    END_FUNCTION
    ");
}

#[rstest]
pub fn comments(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn // comment should stay here
	VAR_INPUT
		(* This one stays on top *)
		IN: BOOL; /* This comment stays right */
	END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn // comment should stay here
    	VAR_INPUT
    		(* This one stays on top *)
    		IN: BOOL; /* This comment stays right */
    	END_VAR
    END_FUNCTION
    ");
}

#[rstest]
pub fn function_call_single_line(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	dfgdfg(a := 1,b:=2,c:=3)
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn dfgdfg(a := 1, b := 2, c := 3)
    END_FUNCTION
    ");
}

#[rstest]
pub fn function_call_multi_line(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	dfgdfg(
	    a := 1,
					b:=2,
				c:=3)
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn
    	dfgdfg(
    		a := 1,
    		b := 2,
    		c := 3
    	)
    END_FUNCTION
    ");
}

#[rstest]
pub fn invocation_single_line(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
	THIS.dfgdfg(a := 1,      b:=2,    c:=3)
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION_BLOCK fb1 THIS.dfgdfg(a := 1, b := 2, c := 3);
    END_FUNCTION_BLOCK
    ");
}

#[rstest]
pub fn invocation_multi_line(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
	THIS.dfgdfg(
	    a := 1,
					b:=2,
				c:=3)
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION_BLOCK fb1
    	THIS.dfgdfg(
    		a := 1,
    		b := 2,
    		c := 3
    	);
    END_FUNCTION_BLOCK
    ");
}

#[rstest]
pub fn init_expr_single_line(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine: STRUCT
		power: INT;
		oil: REAL;
	END_STRUCT
        END_TYPE

        FUNCTION StartEngine
	VAR Base: Engine := (power := 100, oil := 10.0); END_VAR END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Engine: STRUCT
    		power: INT;
    		oil: REAL;
    	END_STRUCT
    END_TYPE

    FUNCTION StartEngine
    	VAR
    		Base: Engine := (power := 100, oil := 10.0);
    	END_VAR
    END_FUNCTION
    ");
}

#[rstest]
pub fn init_expr_multi_line(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine: STRUCT
		power: INT;
		oil: REAL;
	END_STRUCT
        END_TYPE

        FUNCTION StartEngine
	VAR
		Base: Engine := (power := 100,
		oil := 10.0);
	END_VAR
        END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Engine: STRUCT
    		power: INT;
    		oil: REAL;
    	END_STRUCT
    END_TYPE

    FUNCTION StartEngine
    	VAR
    		Base: Engine := (
    			power := 100,
    			oil := 10.0
    		);
    	END_VAR
    END_FUNCTION
    ");
}

#[rstest]
pub fn type_declarations(mut with_db: RootDatabase) {
    let source = r#"
TYPE  Typ :BOOL;
Type2: ARRAY [0..10, 1..5] OF INT;
Type1: INT
Engine: STRUCT 		power: INT;  oil: REAL;
END_STRUCT
        END_TYPE
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Typ: BOOL;
    	Type2: ARRAY[0..10, 1..5] OF INT;
    	Type1: INT Engine: STRUCT
    		power: INT;
    		oil: REAL;
    	END_STRUCT
    END_TYPE
    ");
}

#[rstest]
pub fn struct_init(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine: STRUCT 		power: INT;  oil: REAL;
END_STRUCT
        END_TYPE
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Engine: STRUCT
    		power: INT;
    		oil: REAL;
    	END_STRUCT
    END_TYPE
    ");
}

#[rstest]
pub fn class_and_fb_with_invocations(mut with_db: RootDatabase) {
    let source = r#"
CLASS base METHOD super_method
		VAR_INPUT
			test: INT;
		END_VAR
	END_METHOD
END_CLASS

FUNCTION_BLOCK fb1 EXTENDS base METHOD decl
	END_METHOD

	THIS.decl();
	SUPER.super_method(test := 0);
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    CLASS base
    	METHOD super_method
    		VAR_INPUT
    			test: INT;
    		END_VAR
    	END_METHOD
    END_CLASS

    FUNCTION_BLOCK fb1 EXTENDS base
    	METHOD decl
    	END_METHOD

    	THIS.decl();
    	SUPER.super_method(test := 0);
    END_FUNCTION_BLOCK
    ");
}


#[rstest]
pub fn namespaces_and_using_directives(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1 NAMESPACE nested 
	USING nh, llkn
    USING ng,
    	  	        df
    USING j;
	FUNCTION dffd: BOOL
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE

NAMESPACE ns2
END_NAMESPACE

"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    NAMESPACE ns1
    	NAMESPACE nested
    		USING nh, llkn;
    		USING ng,
    			df;
    		USING j;
    		FUNCTION dffd: BOOL
    		END_FUNCTION
    	END_NAMESPACE
    END_NAMESPACE

    NAMESPACE ns2
    END_NAMESPACE
    ");
}
