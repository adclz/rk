use crate::tests::utils::{add_source, with_db};
use auto_lsp::core::document::Document;
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
            skip_idempotence: false,
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

    let document = add_source(&mut with_db, source).document(&with_db);

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

    	VAR_EXTERNAL CONSTANT
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

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    CLASS CCounter
    	VAR
    		m_iCurrentValue: INT; (* Default = 0 *)
    		m_bCountUp: BOOL := TRUE;
    	END_VAR

    	VAR PUBLIC
    		m_iUpperLimit: INT := +10000;
    		m_iLowerLimit: INT := -10000;
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
pub fn class_definition2(mut with_db: RootDatabase) {
    let source = r#"
 CLASS COUNTER
VAR
CV: UINT;
Max: UINT:= 1000;
END_VAR
// Current value of counter
METHOD PUBLIC UP: UINT
VAR_INPUT INC: UINT; END_VAR
VAR_OUTPUT QU: BOOL; END_VAR
IF CV <= Max - INC
THEN CV:= CV + INC;
QU:= FALSE;
ELSE QU:= TRUE;
END_IF
UP:= CV;
END_METHOD
METHOD PUBLIC UP5: UINT
VAR_OUTPUT QU: BOOL; END_VAR
UP5:= THIS.UP(INC:= 5, QU => QU);
END_METHOD
END_CLASS
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    CLASS COUNTER
    	VAR
    		CV: UINT;
    		Max: UINT := 1000;
    	END_VAR
    	// Current value of counter
    	METHOD PUBLIC UP: UINT
    		VAR_INPUT
    			INC: UINT;
    		END_VAR
    		VAR_OUTPUT
    			QU: BOOL;
    		END_VAR
    		IF CV <= Max - INC THEN
    			CV := CV + INC;
    			QU := FALSE;
    		ELSE
    			QU := TRUE;
    		END_IF;
    		UP := CV;
    	END_METHOD
    	METHOD PUBLIC UP5: UINT
    		VAR_OUTPUT
    			QU: BOOL;
    		END_VAR
    		UP5 := THIS.UP(INC := 5, QU => QU);
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

    let document = add_source(&mut with_db, source).document(&with_db);

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
    	END_CASE;
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

    let document = add_source(&mut with_db, source).document(&with_db);

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

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn
    	J := 101;
    	FOR I := 1 TO 100 BY 2 DO
    		IF WORDS[I] = 'KEY' THEN
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

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn
    	J := -1;
    	REPEAT J := J + 2;
    		UNTIL J = 101 OR WORDS[J] = 'KEY'
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

    let document = add_source(&mut with_db, source).document(&with_db);

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

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn
    	dfgdfg(a := 1, b := 2, c := 3);
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

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn
    	dfgdfg(
    		a := 1,
    		b := 2,
    		c := 3
    	);
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

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION_BLOCK fb1

    	THIS.dfgdfg(a := 1, b := 2, c := 3);

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

    let document = add_source(&mut with_db, source).document(&with_db);

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
		power: INT
		oil: REAL
	END_STRUCT
        END_TYPE

        FUNCTION StartEngine
	VAR Base: Engine := (power := 100, oil := 10.0); END_VAR END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Engine: STRUCT
    		power: INT;
    		oil: REAL;
    	END_STRUCT;
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

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Engine: STRUCT
    		power: INT;
    		oil: REAL;
    	END_STRUCT;
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

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Typ: BOOL;
    	Type2: ARRAY[0..10, 1..5] OF INT;
    	Type1: INT;
    	Engine: STRUCT
    		power: INT;
    		oil: REAL;
    	END_STRUCT;
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

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Engine: STRUCT
    		power: INT;
    		oil: REAL;
    	END_STRUCT;
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

    let document = add_source(&mut with_db, source).document(&with_db);

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

    let document = add_source(&mut with_db, source).document(&with_db);

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

#[rstest]
pub fn single_line_path_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION dffd: BOOL

    test . sdf . sdf   [   0 ] . dd

END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION dffd: BOOL

    	test.sdf.sdf[0].dd

    END_FUNCTION
    ");
}

#[rstest]
pub fn references(mut with_db: RootDatabase) {
    let source = r#"
TYPE
S1: STRUCT
SC1: INT;
SC2: REAL;
END_STRUCT;
A1: ARRAY[1..99] OF INT;
END_TYPE

FUNCTION_BLOCK fb1
VAR
myS1: S1;
myA1: A1;
myRefS1: REF_TO S1:= REF(myS1);
myRefA1: REF_TO A1:= REF(myA1);
myRefInt: REF_TO INT:= REF(myA1[1]);
END_VAR
myRefS1^.SC1:= myRefA1^[12]; // in this case, equivalent to S1.SC1:= A1[12];
myRefInt:= REF(A1[11]);
S1.SC1:= myRefInt^; // assigns the value of A1[11] to S1.SC1

END_FUNCTION_BLOCK
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	S1: STRUCT
    		SC1: INT;
    		SC2: REAL;
    	END_STRUCT;
    	A1: ARRAY[1..99] OF INT;
    END_TYPE

    FUNCTION_BLOCK fb1
    	VAR
    		myS1: S1;
    		myA1: A1;
    		myRefS1: REF_TO S1 := REF(myS1);
    		myRefA1: REF_TO A1 := REF(myA1);
    		myRefInt: REF_TO INT := REF(myA1[1]);
    	END_VAR
    	myRefS1^.SC1 := myRefA1^[12]; // in this case, equivalent to S1.SC1:= A1[12];
    	myRefInt := REF(A1[11]);
    	S1.SC1 := myRefInt^;
    	// assigns the value of A1[11] to S1.SC1


    END_FUNCTION_BLOCK
    ");
}

#[rstest]
pub fn dw_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
VAR
test: REAL;
END_VAR

test := %IX0.0
A := COUNTER.UP( 
        ENO=> %MX1
        ); 

END_FUNCTION_BLOCK
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION_BLOCK fb1
    	VAR
    		test: REAL;
    	END_VAR

    	test := %IX0.0;
    	A := COUNTER.UP(
    		ENO => %MX1
    	);

    END_FUNCTION_BLOCK
    ");
}

#[rstest]
pub fn program_with_dw_variables(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM myPrg
    VAR_ACCESS
        ABLE: STATION_1.%IX1.1: BOOL READ_ONLY;
        BAKER: STATION_1.P1.x2: UINT READ_WRITE;
    END_VAR
END_PROGRAM
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    PROGRAM myPrg
    	VAR_ACCESS
    		ABLE: STATION_1.%IX1.1: BOOL READ_ONLY;
    		BAKER: STATION_1.P1.x2: UINT READ_WRITE;
    	END_VAR
    END_PROGRAM
    ");
}

#[rstest]
pub fn blank_line_between_declarations_in_namespace(mut with_db: RootDatabase) {
    // Blank line between NAMESPACE header and FUNCTION should be preserved
    let source = r#"
NAMESPACE ns

FUNCTION fn1 : INT
END_FUNCTION

FUNCTION fn2 : INT
END_FUNCTION
END_NAMESPACE
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    NAMESPACE ns

    	FUNCTION fn1: INT
    	END_FUNCTION

    	FUNCTION fn2: INT
    	END_FUNCTION
    END_NAMESPACE
    ");
}

#[rstest]
pub fn blank_line_between_top_level_declarations(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
END_FUNCTION

FUNCTION fn2 : INT
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn1: INT
    END_FUNCTION

    FUNCTION fn2: INT
    END_FUNCTION
    ");
}

#[rstest]
pub fn any_type_spec_formatting(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn2 : ANY_NUM
    VAR_INPUT
        x :  ANY_NUM ;
    END_VAR
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn2: ANY_NUM
    	VAR_INPUT
    		x: ANY_NUM;
    	END_VAR
    END_FUNCTION
    ");
}

#[rstest]
pub fn configuration_and_resource(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    VAR_GLOBAL
        missing : INT;
        m: INT;
    END_VAR

    RESOURCE res ON CPU1

        TASK t2(PRIORITY := 1);
            PROGRAM RETAIN inst22 WITH t2 : MyProg; 

        TASK task(PRIORITY := 0)
    
    END_RESOURCE
END_CONFIGURATION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    CONFIGURATION MyCfg
    	VAR_GLOBAL
    		missing: INT;
    		m: INT;
    	END_VAR
    	RESOURCE res ON CPU1

    		TASK t2(PRIORITY := 1);
    		PROGRAM RETAIN inst22 WITH t2: MyProg;

    		TASK task(PRIORITY := 0);

    	END_RESOURCE
    END_CONFIGURATION
    ");
}

#[rstest]
pub fn empty_pou_no_double_blank_line(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn1
	VAR
	
	END_VAR

END_FUNCTION_BLOCK
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION_BLOCK fn1
    	VAR
    	END_VAR

    END_FUNCTION_BLOCK
    ");
}

#[rstest]
pub fn string_literal_single_quote_preserved(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    x := 'Hello World';
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn1
    	x := 'Hello World';
    END_FUNCTION
    ");
}

#[rstest]
pub fn string_literal_double_quote_preserved(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    x := "Hello World";
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r#"
    FUNCTION fn1
    	x := "Hello World";
    END_FUNCTION
    "#);
}

#[rstest]
pub fn string_literal_with_special_chars(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    x := 'It$'s a test $0A';
    y := "double$"quote";
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r#"
    FUNCTION fn1
    	x := 'It$'s a test $0A';
    	y := "double$"quote";
    END_FUNCTION
    "#);
}

#[rstest]
pub fn empty_string_preserved(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    x := '';
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn1
    	x := '';
    END_FUNCTION
    ");
}

#[rstest]
pub fn numeric_literals_preserved(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    VAR
        a: DWORD;
        b: DWORD;
        c: INT;
        d: INT;
        e: REAL;
    END_VAR

    a := 16#FF00FF00;
    b := 16#DEADBEEF;
    c := 2#1010_0101;
    d := 8#777;
    e := 2E-3;

END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn1
    	VAR
    		a: DWORD;
    		b: DWORD;
    		c: INT;
    		d: INT;
    		e: REAL;
    	END_VAR

    	a := 16#FF00FF00;
    	b := 16#DEADBEEF;
    	c := 2#1010_0101;
    	d := 8#777;
    	e := 2E-3;

    END_FUNCTION
    ");
}

#[rstest]
pub fn negative_integers_preserved(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    VAR
        a: INT;
        b: REAL;
    END_VAR

    a := -1;
    b := -3.14;
    a := 10 + -2;

END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn1
    	VAR
    		a: INT;
    		b: REAL;
    	END_VAR

    	a := -1;
    	b := -3.14;
    	a := 10 + -2;

    END_FUNCTION
    ");
}

#[rstest]
pub fn keyword_operators_spacing(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    VAR
        a: BOOL;
        b: DWORD;
        c: INT;
    END_VAR

    a := a XOR TRUE;
    b := b AND b OR b XOR b;
    c := c MOD 3;
    c := c * 2 MOD 5;

END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn1
    	VAR
    		a: BOOL;
    		b: DWORD;
    		c: INT;
    	END_VAR

    	a := a XOR TRUE;
    	b := b AND b OR b XOR b;
    	c := c MOD 3;
    	c := c * 2 MOD 5;

    END_FUNCTION
    ");
}

#[rstest]
pub fn var_qualifier_on_same_line(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : REAL
    VAR CONSTANT
        A: REAL := 3.14;
        B: REAL := 2.71;
    END_VAR
    VAR
        x: REAL;
    END_VAR

    x := A + B;
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION fn1: REAL
    	VAR CONSTANT
    		A: REAL := 3.14;
    		B: REAL := 2.71;
    	END_VAR
    	VAR
    		x: REAL;
    	END_VAR

    	x := A + B;
    END_FUNCTION
    ");
}

#[rstest]
pub fn var_retain_and_non_retain_qualifiers(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR_INPUT RETAIN
        a: INT;
    END_VAR

    VAR_INPUT NON_RETAIN
        b: INT;
    END_VAR

    VAR_OUTPUT RETAIN
        c: INT;
    END_VAR

    VAR_OUTPUT NON_RETAIN
        d: INT;
    END_VAR

    VAR RETAIN
        e: INT;
    END_VAR

    VAR NON_RETAIN
        f: INT;
    END_VAR

    VAR_GLOBAL CONSTANT
        g: INT;
    END_VAR

    VAR_GLOBAL RETAIN
        h: INT;
    END_VAR

    VAR_EXTERNAL CONSTANT
        i: INT;
    END_VAR
END_FUNCTION_BLOCK
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION_BLOCK fb1
    	VAR_INPUT RETAIN
    		a: INT;
    	END_VAR

    	VAR_INPUT NON_RETAIN
    		b: INT;
    	END_VAR

    	VAR_OUTPUT RETAIN
    		c: INT;
    	END_VAR

    	VAR_OUTPUT NON_RETAIN
    		d: INT;
    	END_VAR
    	VAR RETAIN
    		e: INT;
    	END_VAR
    	VAR NON_RETAIN
    		f: INT;
    	END_VAR
    	VAR_GLOBAL CONSTANT
    		g: INT;
    	END_VAR
    	VAR_GLOBAL RETAIN
    		h: INT;
    	END_VAR

    	VAR_EXTERNAL CONSTANT
    		i: INT;
    	END_VAR
    END_FUNCTION_BLOCK
    ");
}

#[rstest]
pub fn extern_pragma_formatting(mut with_db: RootDatabase) {
    // Bad formatting: missing spaces
    let source = r#"
{extern'math''abs'}
FUNCTION test : INT
VAR_INPUT IN : INT; END_VAR
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    {extern 'math' 'abs'}
    FUNCTION test: INT
    	VAR_INPUT
    		IN: INT;
    	END_VAR
    END_FUNCTION
    ");
}

#[rstest]
pub fn test_pragma_formatting(mut with_db: RootDatabase) {
    let source = r#"
{test}
FUNCTION my_test : INT
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    {test}
    FUNCTION my_test: INT
    END_FUNCTION
    ");
}

/// `{export}` and `{once}` each take their own line, like `{test}`, and two
/// above one FUNCTION do not glue onto each other.
#[rstest]
pub fn export_pragma_formatting(mut with_db: RootDatabase) {
    let source = r#"
{once}   {export}
FUNCTION   Setup : INT
    Setup := 1;
END_FUNCTION

NAMESPACE Plant
    // Called by the host on a recipe change.
{export}   FUNCTION Reset : INT
Reset := 0;
END_FUNCTION

{once}
{export}
FUNCTION Other : INT
Other := 0;
END_FUNCTION
END_NAMESPACE
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    {once}
    {export}
    FUNCTION Setup: INT
    	Setup := 1;
    END_FUNCTION

    NAMESPACE Plant
    	// Called by the host on a recipe change.
    	{export}
    	FUNCTION Reset: INT
    		Reset := 0;
    	END_FUNCTION

    	{once}
    	{export}
    	FUNCTION Other: INT
    		Other := 0;
    	END_FUNCTION
    END_NAMESPACE
    ");
}

#[rstest]
pub fn test_pragma_on_program(mut with_db: RootDatabase) {
    let source = r#"
{test}
PROGRAM my_test
    x := 1;
END_PROGRAM
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    {test}
    PROGRAM my_test
    	x := 1;
    END_PROGRAM
    ");
}

#[rstest]
pub fn no_double_blank_line_before_end_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Foo

    FUNCTION_BLOCK Bar
    VAR
        x: INT;
    END_VAR
    END_FUNCTION_BLOCK

END_NAMESPACE
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    NAMESPACE Foo

    	FUNCTION_BLOCK Bar
    		VAR
    			x: INT;
    		END_VAR
    	END_FUNCTION_BLOCK

    END_NAMESPACE
    ");
}

#[rstest]
pub fn long_chained_expression(mut with_db: RootDatabase) {
    // Only one line break in the chain — formatter should break ALL OR operators
    let source = r#"
FUNCTION REVERSE: BYTE
    VAR_INPUT
        IN: BYTE;
    END_VAR

    REVERSE :=
    SHL(IN, 7) OR
    SHR(IN, 7) OR (ROR(IN, 3) AND 2#01000100) OR (ROL(IN, 3) AND 2#00100010) OR (SHL(IN, 1) AND 2#00010000) OR (SHR(IN, 1) AND 2#00001000);
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION REVERSE: BYTE
    	VAR_INPUT
    		IN: BYTE;
    	END_VAR

    	REVERSE := SHL(IN, 7)
    		OR SHR(IN, 7)
    		OR (ROR(IN, 3) AND 2#01000100)
    		OR (ROL(IN, 3) AND 2#00100010)
    		OR (SHL(IN, 1) AND 2#00010000)
    		OR (SHR(IN, 1) AND 2#00001000);
    END_FUNCTION
    ");
}

#[rstest]
pub fn enum_type_formatting(mut with_db: RootDatabase) {
    let source = r#"
TYPE Color : (Red, Green, Blue)
END_TYPE

TYPE Status : INT (Running := 1, Stopped := 2, Error := 3)
END_TYPE
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Color: (Red, Green, Blue);
    END_TYPE

    TYPE
    	Status: INT(Running := 1, Stopped := 2, Error := 3);
    END_TYPE
    ");
}

#[rstest]
pub fn enum_value_formatting(mut with_db: RootDatabase) {
    let source = r#"
TYPE Color : (Red, Green, Blue)
END_TYPE

FUNCTION test : INT
VAR
    c : Color := Color#Red;
END_VAR
    c := Color#Green;
    test := 0;
END_FUNCTION
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    TYPE
    	Color: (Red, Green, Blue);
    END_TYPE

    FUNCTION test: INT
    	VAR
    		c: Color := Color#Red;
    	END_VAR
    	c := Color#Green;
    	test := 0;
    END_FUNCTION
    ");
}

#[rstest]
pub fn allow_pragma_survives_formatting(mut with_db: RootDatabase) {
    // Both positions: above the POU, above a METHOD, and as a statement —
    // the formatter must keep each on its own line and not touch the names.
    let source = r#"
{allow 'input-assignment'   'self-assignment'}
FUNCTION_BLOCK rebinder
VAR_INPUT x: INT; END_VAR
{allow 'dead-code'}
METHOD poke: INT
{allow   'unused-variable'}
poke := 1;
END_METHOD
x := x;
END_FUNCTION_BLOCK
"#;

    let document = add_source(&mut with_db, source).document(&with_db);

    assert_snapshot!(fmt(document), @r"
    {allow 'input-assignment' 'self-assignment'}
    FUNCTION_BLOCK rebinder
    	VAR_INPUT
    		x: INT;
    	END_VAR
    	{allow 'dead-code'}
    	METHOD poke: INT
    		{allow 'unused-variable'}
    		poke := 1;
    	END_METHOD
    	x := x;
    END_FUNCTION_BLOCK
    ");
}

/// The gate every caller shares. Topiary refused ERROR nodes but formatted a
/// file whose only defect was a MISSING node, which is what `check` reports
/// as E0002; both are refused now, and the message says where.
#[rstest]
#[case::missing_node(
    "FUNCTION f : INT\nVAR acc : INT; END_VAR\n    f := acc + INT#some_call(1, 2, 3);\nEND_FUNCTION\n",
    "line 3"
)]
#[case::error_node("FUNCTION f : INT\n    f := ;;; @@ 1\nEND_FUNCTION\n", "line 2")]
fn an_unparseable_source_is_refused(#[case] source: &str, #[case] where_: &str) {
    let err = formatter::format_source(source).expect_err("must refuse");
    let msg = err.to_string();
    assert!(msg.starts_with("syntax error at "), "{msg}");
    assert!(msg.contains(where_), "names the line: {msg}");
}

#[rstest]
fn a_parseable_source_is_formatted() {
    let out = formatter::format_source("FUNCTION g : INT  g := 2; END_FUNCTION\n").unwrap();
    assert!(out.contains("END_FUNCTION"));
    assert_ne!(out, "FUNCTION g : INT  g := 2; END_FUNCTION\n", "reflowed");
}

/// Every place the grammar makes the terminator optional, written without
/// one. The formatter supplies it, and supplies it once: each case is
/// formatted again from its own output and has to land on the same text.
#[rstest]
pub fn a_missing_terminator_is_supplied_once() {
    const CASES: &[(&str, &str)] = &[
        (
            "type declarations",
            r#"
TYPE t : INT
u : BOOL
END_TYPE
"#,
        ),
        (
            "struct elements",
            r#"
TYPE r :
STRUCT
    a : INT
    b : BOOL
END_STRUCT
END_TYPE
"#,
        ),
        (
            "variable declarations",
            r#"
FUNCTION_BLOCK fb
VAR_INPUT
    a : INT
END_VAR
VAR
    b : BOOL
END_VAR
END_FUNCTION_BLOCK
"#,
        ),
        (
            "a using directive",
            r#"
NAMESPACE ns
END_NAMESPACE
USING ns
"#,
        ),
        (
            "one naming several",
            r#"
NAMESPACE a
END_NAMESPACE
NAMESPACE b
END_NAMESPACE
USING a, b
"#,
        ),
        (
            "assignments and calls",
            r#"
FUNCTION g : INT
END_FUNCTION

FUNCTION f : INT
VAR
    x : INT;
END_VAR
    x := 1
    g()
    RETURN
END_FUNCTION
"#,
        ),
        (
            "inside a case branch",
            r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    CASE x OF
    1: x := 1
    ELSE
        x := 2
    END_CASE
END_FUNCTION
"#,
        ),
        (
            "inside loops",
            r#"
FUNCTION f : INT
VAR
    i : INT;
END_VAR
    FOR i := 1 TO 3 DO
        WHILE TRUE DO
            EXIT
        END_WHILE
    END_FOR
END_FUNCTION
"#,
        ),
        (
            "configuration members",
            r#"
PROGRAM prog
END_PROGRAM

CONFIGURATION c
VAR_GLOBAL
    g : INT
END_VAR
RESOURCE r ON cpu
    TASK t(PRIORITY := 1)
    PROGRAM p WITH t : prog
END_RESOURCE
END_CONFIGURATION
"#,
        ),
        (
            "already terminated",
            r#"
FUNCTION_BLOCK fb
VAR
    a : INT;
END_VAR
    a := 1;
END_FUNCTION_BLOCK
"#,
        ),
    ];

    assert_snapshot!(formatted(CASES), @r"
    --- type declarations
    TYPE
    	t: INT;
    	u: BOOL;
    END_TYPE
    --- struct elements
    TYPE
    	r: STRUCT
    		a: INT;
    		b: BOOL;
    	END_STRUCT;
    END_TYPE
    --- variable declarations
    FUNCTION_BLOCK fb
    	VAR_INPUT
    		a: INT;
    	END_VAR
    	VAR
    		b: BOOL;
    	END_VAR
    END_FUNCTION_BLOCK
    --- a using directive
    NAMESPACE ns
    END_NAMESPACE
    USING ns;
    --- one naming several
    NAMESPACE a
    END_NAMESPACE
    NAMESPACE b
    END_NAMESPACE
    USING a, b;
    --- assignments and calls
    FUNCTION g: INT
    END_FUNCTION

    FUNCTION f: INT
    	VAR
    		x: INT;
    	END_VAR
    	x := 1;
    	g();
    	RETURN;
    END_FUNCTION
    --- inside a case branch
    FUNCTION f: INT
    	VAR
    		x: INT;
    	END_VAR
    	CASE x OF
    		1: x := 1;
    	ELSE
    		x := 2;
    	END_CASE;
    END_FUNCTION
    --- inside loops
    FUNCTION f: INT
    	VAR
    		i: INT;
    	END_VAR
    	FOR i := 1 TO 3 DO
    		WHILE TRUE DO
    			EXIT
    		END_WHILE;
    	END_FOR;
    END_FUNCTION
    --- configuration members
    PROGRAM prog
    END_PROGRAM
    CONFIGURATION c
    	VAR_GLOBAL
    		g: INT;
    	END_VAR
    	RESOURCE r ON cpu
    		TASK t(PRIORITY := 1);
    		PROGRAM p WITH t: prog;
    	END_RESOURCE
    END_CONFIGURATION
    --- already terminated
    FUNCTION_BLOCK fb
    	VAR
    		a: INT;
    	END_VAR
    	a := 1;
    END_FUNCTION_BLOCK
    ");
}

/// A USING directive carries its own terminator, so the guard that stops a
/// second one has to look inside the node. Every shape the grammar allows,
/// including the places a directive may stand.
#[rstest]
pub fn a_using_directive_is_terminated_once() {
    const CASES: &[(&str, &str)] = &[
        (
            "one name",
            r#"
NAMESPACE a
END_NAMESPACE
USING a
"#,
        ),
        (
            "several names",
            r#"
NAMESPACE a
END_NAMESPACE
NAMESPACE b
END_NAMESPACE
NAMESPACE c
END_NAMESPACE
USING a, b, c
"#,
        ),
        (
            "a dotted path",
            r#"
NAMESPACE a
NAMESPACE b
END_NAMESPACE
END_NAMESPACE
USING a.b
"#,
        ),
        (
            "a dotted path among names",
            r#"
NAMESPACE a
NAMESPACE b
END_NAMESPACE
END_NAMESPACE
USING a.b, a
"#,
        ),
        (
            "one directive already terminated",
            r#"
NAMESPACE a
END_NAMESPACE
NAMESPACE b
END_NAMESPACE
USING a;
USING b
"#,
        ),
        (
            "inside a namespace",
            r#"
NAMESPACE a
END_NAMESPACE

NAMESPACE outer
USING a
FUNCTION f : INT
END_FUNCTION
END_NAMESPACE
"#,
        ),
        (
            "inside a POU",
            r#"
NAMESPACE a
END_NAMESPACE

FUNCTION_BLOCK fb
USING a
END_FUNCTION_BLOCK
"#,
        ),
    ];

    assert_snapshot!(formatted(CASES), @r"
    --- one name
    NAMESPACE a
    END_NAMESPACE
    USING a;
    --- several names
    NAMESPACE a
    END_NAMESPACE
    NAMESPACE b
    END_NAMESPACE
    NAMESPACE c
    END_NAMESPACE
    USING a, b, c;
    --- a dotted path
    NAMESPACE a
    	NAMESPACE b
    	END_NAMESPACE
    END_NAMESPACE
    USING a.b;
    --- a dotted path among names
    NAMESPACE a
    	NAMESPACE b
    	END_NAMESPACE
    END_NAMESPACE
    USING a.b, a;
    --- one directive already terminated
    NAMESPACE a
    END_NAMESPACE
    NAMESPACE b
    END_NAMESPACE
    USING a;
    USING b;
    --- inside a namespace
    NAMESPACE a
    END_NAMESPACE

    NAMESPACE outer
    	USING a;
    	FUNCTION f: INT
    	END_FUNCTION
    END_NAMESPACE
    --- inside a POU
    NAMESPACE a
    END_NAMESPACE

    FUNCTION_BLOCK fb
    	USING a;
    END_FUNCTION_BLOCK
    ");
}

/// A CASE branch holds a statement list, and the statement itself takes a
/// terminator after END_CASE. An empty branch has nothing to terminate.
#[rstest]
pub fn a_case_statement_is_terminated_once() {
    const CASES: &[(&str, &str)] = &[
        (
            "ranges and lists",
            r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    CASE x OF
    1..5: x := 1
    6, 8: x := 2
    END_CASE
END_FUNCTION
"#,
        ),
        (
            "an else branch only",
            r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    CASE x OF
    ELSE
        x := 9
    END_CASE
END_FUNCTION
"#,
        ),
        (
            "an empty branch",
            r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    CASE x OF
    1:
    2: x := 2
    END_CASE
END_FUNCTION
"#,
        ),
        (
            "several statements in a branch",
            r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    CASE x OF
    1:
        x := 1
        x := 2
    END_CASE
END_FUNCTION
"#,
        ),
        (
            "a nested case",
            r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    CASE x OF
    1:
        CASE x OF
        2: x := 2
        END_CASE
    END_CASE
END_FUNCTION
"#,
        ),
        (
            "a branch holding an IF",
            r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    CASE x OF
    1:
        IF TRUE THEN
            x := 1
        END_IF
    END_CASE
END_FUNCTION
"#,
        ),
        (
            "inside a loop",
            r#"
FUNCTION f : INT
VAR
    i : INT;
END_VAR
    FOR i := 1 TO 3 DO
        CASE i OF
        1: i := 1
        END_CASE
    END_FOR
END_FUNCTION
"#,
        ),
        (
            "enum selectors",
            r#"
TYPE m : (a, b); END_TYPE

FUNCTION f : INT
VAR
    x : m;
END_VAR
    CASE x OF
    m#a: x := m#b
    END_CASE
END_FUNCTION
"#,
        ),
        (
            "already terminated",
            r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    CASE x OF
    1: x := 1;
    END_CASE;
END_FUNCTION
"#,
        ),
    ];

    assert_snapshot!(formatted(CASES), @r"
    --- ranges and lists
    FUNCTION f: INT
    	VAR
    		x: INT;
    	END_VAR
    	CASE x OF
    		1..5: x := 1;
    		6, 8: x := 2;
    	END_CASE;
    END_FUNCTION
    --- an else branch only
    FUNCTION f: INT
    	VAR
    		x: INT;
    	END_VAR
    	CASE x OF
    	ELSE
    		x := 9;
    	END_CASE;
    END_FUNCTION
    --- an empty branch
    FUNCTION f: INT
    	VAR
    		x: INT;
    	END_VAR
    	CASE x OF
    		1:
    		2: x := 2;
    	END_CASE;
    END_FUNCTION
    --- several statements in a branch
    FUNCTION f: INT
    	VAR
    		x: INT;
    	END_VAR
    	CASE x OF
    		1:
    			x := 1;
    			x := 2;
    	END_CASE;
    END_FUNCTION
    --- a nested case
    FUNCTION f: INT
    	VAR
    		x: INT;
    	END_VAR
    	CASE x OF
    		1:
    			CASE x OF
    				2: x := 2;
    			END_CASE;
    	END_CASE;
    END_FUNCTION
    --- a branch holding an IF
    FUNCTION f: INT
    	VAR
    		x: INT;
    	END_VAR
    	CASE x OF
    		1:
    			IF TRUE THEN
    				x := 1;
    			END_IF;
    	END_CASE;
    END_FUNCTION
    --- inside a loop
    FUNCTION f: INT
    	VAR
    		i: INT;
    	END_VAR
    	FOR i := 1 TO 3 DO
    		CASE i OF
    			1: i := 1;
    		END_CASE;
    	END_FOR;
    END_FUNCTION
    --- enum selectors
    TYPE
    	m: (a, b);
    END_TYPE

    FUNCTION f: INT
    	VAR
    		x: m;
    	END_VAR
    	CASE x OF
    		m#a: x := m#b;
    	END_CASE;
    END_FUNCTION
    --- already terminated
    FUNCTION f: INT
    	VAR
    		x: INT;
    	END_VAR
    	CASE x OF
    		1: x := 1;
    	END_CASE;
    END_FUNCTION
    ");
}

/// Formats each case and its own output, refusing any that does not land on
/// the same text twice, and renders them for one snapshot.
fn formatted(cases: &[(&str, &str)]) -> String {
    let mut rendered = String::new();
    for (label, source) in cases {
        let once = formatter::format_source(source).expect("formats");
        let twice = formatter::format_source(&once).expect("formats again");
        assert_eq!(once, twice, "{label}: a second pass changed the text");
        assert!(!once.contains(";;"), "{label}: wrote a second terminator");

        rendered.push_str(&format!("--- {label}\n{once}"));
    }
    rendered
}
