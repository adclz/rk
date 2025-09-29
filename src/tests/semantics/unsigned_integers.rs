use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_bool_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: BOOL := TRUE;
        test2: BOOL := FALSE;
        test3: BOOL := 1;
        test4: BOOL := 0;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
#[case("BYTE")]
#[case("USINT")]
fn valid_u8_cases(mut with_db: RootDatabase, #[case] typ: &str) {
    let source = format!(
        r#"
FUNCTION_BLOCK fb1
    VAR
        test1: {typ} := 0;
        test2: {typ} := 255;
        test3: {typ} := USINT#120;
        test7: {typ} := BYTE#10;
        test4: {typ} := 2#0101;
        test5: {typ} := 8#75;
        test6: {typ} := 16#A;
    END_VAR
END_FUNCTION_BLOCK"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("WORD")]
#[case("UINT")]
fn valid_u16_cases(mut with_db: RootDatabase, #[case] typ: &str) {
    let source = format!(r#"
FUNCTION_BLOCK fb1
    VAR
        test1: {typ} := 0;
        test2: {typ} := 65535;
        test3: {typ} := USINT#120;
        test4: {typ} := 2#0101;
        test5: {typ} := 8#75;
        test6: {typ} := 16#A;
        test7: {typ} := BYTE#10
        test8: {typ} := WORD#10
        test9: {typ} := UINT#10
    END_VAR
END_FUNCTION_BLOCK"#);

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("DWORD")]
#[case("UDINT")]
fn valid_u32_cases(mut with_db: RootDatabase, #[case] typ: &str)  {
    let source = format!(r#"
FUNCTION_BLOCK fb1
    VAR
        test1: {typ} := 0;
        test2: {typ} := 4294967295;
        test3: {typ} := BYTE#120;
        test4: {typ} := 2#0101;
        test5: {typ} := 8#75;
        test6: {typ} := 16#A;
        test7: {typ} := DWORD#10
        test8: {typ} := UDINT#10
    END_VAR
END_FUNCTION_BLOCK"#);

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("LWORD")]
#[case("ULINT")]
fn valid_u64_cases(mut with_db: RootDatabase, #[case] typ: &str)  {
    let source = format!(r#"
FUNCTION_BLOCK fb1
    VAR
        test1: {typ} := 0;
        test2: {typ} := 18446744073709551615;
        test3: {typ} := BYTE#120;
        test4: {typ} := 2#0101;
        test5: {typ} := 8#75;
        test6: {typ} := 16#A;
        test7: {typ} := LWORD#10
        test8: {typ} := ULINT#10
    END_VAR
END_FUNCTION_BLOCK"#);

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
fn invalid_bool_literal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: BOOL := 256;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:23 ]
       |
     4 |         test: BOOL := 256;
       |         ^^|^  ^^|^    ^|^  
       |           `---------------- 'test' is declared here
       |                 |      |   
       |                 `---------- type defined here
       |                        |   
       |                        `--- invalid value initializer: invalid BOOL literal
    ---'
    ");
}

#[rstest]
fn invalid_u8_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: USINT := -1;
        test2: BYTE := 256;
        test3: USINT := 16#FFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: USINT := -1;
       |         ^^|^^  ^^|^^    ^|  
       |           `----------------- 'test1' is declared here
       |                  |       |  
       |                  `---------- type defined here
       |                          |  
       |                          `-- invalid value initializer: literal can not be negative
    ---'
    Error: 
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: BYTE := 256;
       |         ^^|^^  ^^|^    ^|^  
       |           `----------------- 'test2' is declared here
       |                  |      |   
       |                  `---------- type defined here
       |                         |   
       |                         `--- invalid value initializer: number too large to fit in target type
    ---'
    Error: 
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: USINT := 16#FFFF;
       |         ^^|^^  ^^|^^    ^^^|^^^  
       |           `---------------------- 'test3' is declared here
       |                  |         |     
       |                  `--------------- type defined here
       |                            |     
       |                            `----- invalid value initializer: number too large to fit in target type
    ---'
    ");
}

#[rstest]
fn invalid_u16_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: UINT := -1;
        test2: WORD := 65536;
        test3: UINT := 16#FFFFFFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:24 ]
       |
     4 |         test1: UINT := -1;
       |         ^^|^^  ^^|^    ^|  
       |           `---------------- 'test1' is declared here
       |                  |      |  
       |                  `--------- type defined here
       |                         |  
       |                         `-- invalid value initializer: literal can not be negative
    ---'
    Error: 
       ,-[ file:///test0.st:5:24 ]
       |
     5 |         test2: WORD := 65536;
       |         ^^|^^  ^^|^    ^^|^^  
       |           `------------------- 'test2' is declared here
       |                  |       |    
       |                  `------------ type defined here
       |                          |    
       |                          `---- invalid value initializer: number too large to fit in target type
    ---'
    Error: 
       ,-[ file:///test0.st:6:24 ]
       |
     6 |         test3: UINT := 16#FFFFFFFF;
       |         ^^|^^  ^^|^    ^^^^^|^^^^^  
       |           `------------------------- 'test3' is declared here
       |                  |          |       
       |                  `------------------ type defined here
       |                             |       
       |                             `------- invalid value initializer: number too large to fit in target type
    ---'
    ");
}

#[rstest]
fn invalid_u32_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: UDINT := -1;
        test2: DWORD := 4294967296;
        test3: UDINT := 16#FFFFFFFFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: UDINT := -1;
       |         ^^|^^  ^^|^^    ^|  
       |           `----------------- 'test1' is declared here
       |                  |       |  
       |                  `---------- type defined here
       |                          |  
       |                          `-- invalid value initializer: literal can not be negative
    ---'
    Error: 
       ,-[ file:///test0.st:5:25 ]
       |
     5 |         test2: DWORD := 4294967296;
       |         ^^|^^  ^^|^^    ^^^^^|^^^^  
       |           `------------------------- 'test2' is declared here
       |                  |           |      
       |                  `------------------ type defined here
       |                              |      
       |                              `------ invalid value initializer: number too large to fit in target type
    ---'
    Error: 
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: UDINT := 16#FFFFFFFFFF;
       |         ^^|^^  ^^|^^    ^^^^^^|^^^^^^  
       |           `---------------------------- 'test3' is declared here
       |                  |            |        
       |                  `--------------------- type defined here
       |                               |        
       |                               `-------- invalid value initializer: number too large to fit in target type
    ---'
    ");
}

#[rstest]
fn invalid_u64_cases(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test1: ULINT := -1;
        test2: LWORD := 18446744073709551616;
        test3: ULINT := 16#FFFFFFFFFFFFFFFFFF;
    END_VAR
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:25 ]
       |
     4 |         test1: ULINT := -1;
       |         ^^|^^  ^^|^^    ^|  
       |           `----------------- 'test1' is declared here
       |                  |       |  
       |                  `---------- type defined here
       |                          |  
       |                          `-- invalid value initializer: literal can not be negative
    ---'
    Error: 
       ,-[ file:///test0.st:5:25 ]
       |
     5 |         test2: LWORD := 18446744073709551616;
       |         ^^|^^  ^^|^^    ^^^^^^^^^^|^^^^^^^^^  
       |           `----------------------------------- 'test2' is declared here
       |                  |                |           
       |                  `---------------------------- type defined here
       |                                   |           
       |                                   `----------- invalid value initializer: number too large to fit in target type
    ---'
    Error: 
       ,-[ file:///test0.st:6:25 ]
       |
     6 |         test3: ULINT := 16#FFFFFFFFFFFFFFFFFF;
       |         ^^|^^  ^^|^^    ^^^^^^^^^^|^^^^^^^^^^  
       |           `------------------------------------ 'test3' is declared here
       |                  |                |            
       |                  `----------------------------- type defined here
       |                                   |            
       |                                   `------------ invalid value initializer: number too large to fit in target type
    ---'
    ");
}
