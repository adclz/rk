use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_access_private_method(mut with_db: RootDatabase) {
    let source = r#"
    CLASS Base
        METHOD PRIVATE myPrivateMethod: INT  END_METHOD

        METHOD testMethod: INT
            THIS.myPrivateMethod();
        END_METHOD
    END_CLASS
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_access_private_method(mut with_db: RootDatabase) {
    let source = r#"
    CLASS Base
        METHOD PRIVATE myPrivateMethod: INT  END_METHOD
    END_CLASS

    CLASS Mid EXTENDS Base
        METHOD testMethod: INT
            SUPER.myPrivateMethod();
        END_METHOD
    END_CLASS
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: access control violation
       ,-[ file:///test0.st:8:19 ]
       |
     8 |             SUPER.myPrivateMethod();
       |                   ^^^^^^^|^^^^^^^  
       |                          `--------- can not access PRIVATE item 'myPrivateMethod'
       | 
       | Note: variables and methods marked PRIVATE can only be accessed from within the same POU
    ---'
    ");
}

#[rstest]
fn access_internal_method_from_different_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    CLASS Base
        METHOD INTERNAL myInternalMethod: INT  END_METHOD
    END_CLASS
END_NAMESPACE

NAMESPACE ns2
    USING ns1;

    CLASS Mid EXTENDS Base
        METHOD testMethod: INT
            SUPER.myInternalMethod();
        END_METHOD
    END_CLASS
END_NAMESPACE
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0402] Error: access control violation
        ,-[ file:///test0.st:13:19 ]
        |
     13 |             SUPER.myInternalMethod();
        |                   ^^^^^^^^|^^^^^^^  
        |                           `--------- can not access INTERNAL item 'myInternalMethod'
        | 
        | Note: calling scope is in NAMESPACE 'ns2', item is only available in NAMESPACE 'ns1'
    ----'
    ");
}

#[rstest]
fn access_internal_method_in_namespace_from_global_scope(mut with_db: RootDatabase) {
    let source = r#"
USING ns2;

CLASS Mid EXTENDS Base
    METHOD testMethod: INT
        SUPER.myInternalMethod();
    END_METHOD
END_CLASS

NAMESPACE ns2
    CLASS Base
        METHOD INTERNAL myInternalMethod: INT  END_METHOD
    END_CLASS
END_NAMESPACE
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0402] Error: access control violation
       ,-[ file:///test0.st:6:15 ]
       |
     6 |         SUPER.myInternalMethod();
       |               ^^^^^^^^|^^^^^^^  
       |                       `--------- can not access INTERNAL item 'myInternalMethod'
       | 
       | Note: calling scope is in the GLOBAL scope, item is only available in NAMESPACE 'ns2'
    ---'
    ");
}

#[rstest]
fn access_internal_method_in_global_scope_from_namespace(mut with_db: RootDatabase) {
    let source = r#"
CLASS Base
    METHOD INTERNAL myInternalMethod: INT  END_METHOD
END_CLASS

NAMESPACE ns2
    CLASS Mid EXTENDS Base
        METHOD testMethod: INT
            SUPER.myInternalMethod();
        END_METHOD
    END_CLASS
END_NAMESPACE
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0402] Error: access control violation
       ,-[ file:///test0.st:9:19 ]
       |
     9 |             SUPER.myInternalMethod();
       |                   ^^^^^^^^|^^^^^^^  
       |                           `--------- can not access INTERNAL item 'myInternalMethod'
       | 
       | Note: calling scope is in NAMESPACE 'ns2', item scope is only available the GLOBAL scope
    ---'
    ");
}

#[rstest]
fn valid_access_protected_method_in_derived_pou(mut with_db: RootDatabase) {
    let source = r#"
CLASS Base
    METHOD PROTECTED myProtectedMethod: INT  END_METHOD
END_CLASS

FUNCTION_BLOCK fn1 EXTENDS Base
    VAR
        obj: Base;
    END_VAR

    SUPER.myProtectedMethod();

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn access_protected_method_in_non_derived_pou(mut with_db: RootDatabase) {
    let source = r#"
CLASS Base
    METHOD PROTECTED myProtectedMethod  END_METHOD
END_CLASS

FUNCTION_BLOCK fn1
    VAR
        obj: Base;
    END_VAR

    obj.myProtectedMethod();

END_FUNCTION_BLOCK
    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0403] Error: access control violation
        ,-[ file:///test0.st:11:9 ]
        |
     11 |     obj.myProtectedMethod();
        |         ^^^^^^^^|^^^^^^^^  
        |                 `---------- can not access PROTECTED item 'myProtectedMethod'
        | 
        | Note: Variables and methods marked PROTECTED are only available within the same POU or derived POUs
    ----'
    ");
}
