use std::sync::{Arc, RwLock};

use crate::tests::utils::{add_sources, with_log_db};
use auto_lsp::salsa::Event;
use db::RootDatabase;
use insta::assert_debug_snapshot;
use rstest::rstest;

/*
#[rstest]
fn self_extends_bo_cycle(mut with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>)) {
    let source = r#"
        CLASS MyClass EXTENDS MyClass
        END_CLASS
        "#;

    add_sources(&mut with_log_db.0, &[source]);

    let lock = with_log_db.1.read().unwrap();
    let logs = lock
      .iter().collect::<Vec<_>>();

    assert_debug_snapshot!(logs, @"[]");
}


#[rstest]
fn self_referential(mut with_db: RootDatabase) {
    let source = r#"
      FUNCTION_BLOCK fb
            VAR_INPUT
                invalid : fb;
            END_VAR

        END_FUNCTION_BLOCK

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:22 ]
       |
     2 |       FUNCTION_BLOCK fb
       |                      ^|  
       |                       `-- 'fb' is recursive
    ---'
    Error: 
       ,-[ file:///test0.st:4:17 ]
       |
     2 |       FUNCTION_BLOCK fb
       |                      ^|  
       |                       `-- 'fb' is originally declared here
       | 
     4 |                 invalid : fb;
       |                 ^^^|^^|^^^^^  
       |                    `---------- ... and recurse at this location
       |                       |       
       |                       `------- 'invalid' creates a recursion with 'fb'
    ---'
    ");
}

#[rstest]
fn recursive_function_blocks(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
              VAR_INPUT
                  invalid : fb2;
              END_VAR

        END_FUNCTION_BLOCK

        FUNCTION_BLOCK fb2
              VAR_INPUT
                  invalid : fb1;
              END_VAR

        END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:24 ]
       |
     2 |         FUNCTION_BLOCK fb1
       |                        ^|^  
       |                         `--- 'fb1' is recursive
    ---'
    Error: 
       ,-[ file:///test0.st:4:19 ]
       |
     4 |                   invalid : fb2;
       |                   ^^^|^^|^^^^^^  
       |                      `----------- ... and recurse at this location
       |                         |        
       |                         `-------- 'invalid' creates a recursion with 'fb2'
       | 
     9 |         FUNCTION_BLOCK fb2
       |                        ^|^  
       |                         `--- 'fb2' is originally declared here
    ---'
    Error: 
       ,-[ file:///test0.st:9:24 ]
       |
     9 |         FUNCTION_BLOCK fb2
       |                        ^|^  
       |                         `--- 'fb2' is recursive
    ---'
    Error: 
        ,-[ file:///test0.st:11:19 ]
        |
      2 |         FUNCTION_BLOCK fb1
        |                        ^|^  
        |                         `--- 'fb1' is originally declared here
        | 
     11 |                   invalid : fb1;
        |                   ^^^|^^|^^^^^^  
        |                      `----------- ... and recurse at this location
        |                         |        
        |                         `-------- 'invalid' creates a recursion with 'fb1'
    ----'
    ");
}

#[rstest]
fn self_referential_struct(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine: STRUCT
                sub_engine: Engine;
            END_STRUCT
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:2:14 ]
       |
     2 |         TYPE Engine: STRUCT
       |              ^^^|^^  
       |                 `---- 'Engine' is recursive
    ---'
    Error: 
       ,-[ file:///test0.st:3:17 ]
       |
     2 |         TYPE Engine: STRUCT
       |              ^^^|^^  
       |                 `---- 'Engine' is originally declared here
     3 |                 sub_engine: Engine;
       |                 ^^^^^|^^^|^^^^^^^^  
       |                      `-------------- ... and recurse at this location
       |                          |          
       |                          `---------- 'sub_engine' creates a recursion with 'Engine'
    ---'
    ");
}
*/
