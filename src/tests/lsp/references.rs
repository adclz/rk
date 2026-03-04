use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::Url;
use db::RootDatabase;
use ide_proto::{handlers::ReferencesHandler, walk::descendant_at};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, render_references, with_db};

#[rstest]
fn pou_references(mut with_db: RootDatabase) {
    let source = r#"FUNCTION_BLOCK MyFB
END_FUNCTION_BLOCK

FUNCTION fn1
VAR
    x : MyFB;
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("MyFB").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "MyFB"), @r"
    Advice: 2 reference(s) to 'MyFB'
       ,-[ file:///test0.st:1:16 ]
       |
     1 | FUNCTION_BLOCK MyFB
       |                ^^|^
       |                  `--- 2 reference(s) to 'MyFB'
       |
     6 |     x : MyFB;
       |         ^^|^
       |           `--- reference
    ---'
    ");
}

#[rstest]
fn variable_references(mut with_db: RootDatabase) {
    let source = r#"FUNCTION fn1
VAR
    x : INT;
END_VAR
    x := 1;
    x := x + 2;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("x : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "x"), @r"
    Advice: 4 reference(s) to 'x'
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     x : INT;
       |     |
       |     `-- 4 reference(s) to 'x'
       |
     5 |     x := 1;
       |     |
       |     `-- reference
     6 |     x := x + 2;
       |     |    |
       |     `------- reference
       |          |
       |          `-- reference
    ---'
    ");
}

#[rstest]
fn variable_references_from_usage(mut with_db: RootDatabase) {
    let source = r#"FUNCTION fn1
VAR
    x : INT;
END_VAR
    x := 1;
    x := x + 2;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("x := 1").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "x"), @r"
    Advice: 4 reference(s) to 'x'
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     x : INT;
       |     |
       |     `-- 4 reference(s) to 'x'
       |
     5 |     x := 1;
       |     |
       |     `-- reference
     6 |     x := x + 2;
       |     |    |
       |     `------- reference
       |          |
       |          `-- reference
    ---'
    ");
}

#[rstest]
fn cross_file_pou_references(mut with_db: RootDatabase) {
    let source1 = r#"FUNCTION_BLOCK SharedFB
END_FUNCTION_BLOCK
"#;

    let source2 = r#"FUNCTION user1
VAR
    a : SharedFB;
END_VAR
END_FUNCTION
"#;

    let source3 = r#"FUNCTION user2
VAR
    b : SharedFB;
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source1, source2, source3]);
    let file1 = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();

    let offset = source1.find("SharedFB").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "SharedFB"), @r"
    Advice: 3 reference(s) to 'SharedFB'
       ,-[ file:///test0.st:1:16 ]
       |
     1 | FUNCTION_BLOCK SharedFB
       |                ^^^^|^^^
       |                    `----- 3 reference(s) to 'SharedFB'
       |
       |-[ file:///test1.st:3:9 ]
       |
     3 |     a : SharedFB;
       |         ^^^^|^^^
       |             `----- reference
       |
       |-[ file:///test2.st:3:9 ]
       |
     3 |     b : SharedFB;
       |         ^^^^|^^^
       |             `----- reference
    ---'
    ");
}

#[rstest]
fn no_cross_scope_variable_leak(mut with_db: RootDatabase) {
    let source = r#"FUNCTION fn1
VAR
    x : INT;
END_VAR
    x := 1;
END_FUNCTION

FUNCTION fn2
VAR
    x : INT;
END_VAR
    x := 2;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("x : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "x"), @r"
    Advice: 2 reference(s) to 'x'
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     x : INT;
       |     |
       |     `-- 2 reference(s) to 'x'
       |
     5 |     x := 1;
       |     |
       |     `-- reference
    ---'
    ");
}

#[rstest]
fn namespace_references(mut with_db: RootDatabase) {
    let source1 = r#"NAMESPACE MyNs
    FUNCTION fn1 : INT
    END_FUNCTION
END_NAMESPACE
"#;

    let source2 = r#"NAMESPACE MyNs
    FUNCTION fn2 : INT
    END_FUNCTION
END_NAMESPACE
"#;

    add_sources(&mut with_db, &[source1, source2]);
    let file1 = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();

    let offset = source1.find("MyNs").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "MyNs"), @r"
    Advice: 2 reference(s) to 'MyNs'
       ,-[ file:///test0.st:1:11 ]
       |
     1 | NAMESPACE MyNs
       |           ^^|^
       |             `--- 2 reference(s) to 'MyNs'
       |
       |-[ file:///test1.st:1:11 ]
       |
     1 | NAMESPACE MyNs
       |           ^^|^
       |             `--- reference
    ---'
    ");
}

#[rstest]
fn namespace_references_with_using(mut with_db: RootDatabase) {
    let source1 = r#"NAMESPACE MyNs
    FUNCTION fn1 : INT
    END_FUNCTION
END_NAMESPACE
"#;

    let source2 = r#"USING MyNs;
FUNCTION fn2 : INT
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source1, source2]);
    let file1 = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();

    let offset = source1.find("MyNs").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "MyNs"), @r"
    Advice: 2 reference(s) to 'MyNs'
       ,-[ file:///test0.st:1:11 ]
       |
     1 | NAMESPACE MyNs
       |           ^^|^
       |             `--- 2 reference(s) to 'MyNs'
       |
       |-[ file:///test1.st:1:7 ]
       |
     1 | USING MyNs;
       |       ^^|^
       |         `--- reference
    ---'
    ");
}

#[rstest]
fn struct_field_references(mut with_db: RootDatabase) {
    let source = r#"TYPE
    Engine: STRUCT
        oil: INT;
        fuel: BOOL;
    END_STRUCT
END_TYPE

FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.fuel := my_var.fuel;

    my_var.fuel := 0;

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("fuel: BOOL").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "fuel"), @r"
    Advice: 4 reference(s) to 'fuel'
        ,-[ file:///test0.st:4:9 ]
        |
      4 |         fuel: BOOL;
        |         ^^|^
        |           `--- 4 reference(s) to 'fuel'
        |
     13 |     my_var.fuel := my_var.fuel;
        |            ^^|^           ^^|^
        |              `------------------ reference
        |                             |
        |                             `--- reference
        |
     15 |     my_var.fuel := 0;
        |            ^^|^
        |              `--- reference
    ----'
    ");
}

#[rstest]
fn nested_struct_field_references(mut with_db: RootDatabase) {
    let source = r#"TYPE
    SubEngine: STRUCT
        oil: INT;
        fuel: BOOL;
    END_STRUCT
    Engine: STRUCT
        engine: SubEngine;
    END_STRUCT
END_TYPE

FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.engine.fuel := my_var.engine.fuel;

    my_var.engine.fuel := 0;

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("fuel: BOOL").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "fuel"), @r"
    Advice: 4 reference(s) to 'fuel'
        ,-[ file:///test0.st:4:9 ]
        |
      4 |         fuel: BOOL;
        |         ^^|^
        |           `--- 4 reference(s) to 'fuel'
        |
     16 |     my_var.engine.fuel := my_var.engine.fuel;
        |                   ^^|^                  ^^|^
        |                     `------------------------- reference
        |                                           |
        |                                           `--- reference
        |
     18 |     my_var.engine.fuel := 0;
        |                   ^^|^
        |                     `--- reference
    ----'
    ");

    let offset = source.find("engine: SubEngine").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "engine"), @r"
       Advice: 4 reference(s) to 'engine'
           ,-[ file:///test0.st:7:9 ]
           |
         7 |         engine: SubEngine;
           |         ^^^|^^
           |            `---- 4 reference(s) to 'engine'
           |
        16 |     my_var.engine.fuel := my_var.engine.fuel;
           |            ^^^|^^                ^^^|^^
           |               `-------------------------- reference
           |                                     |
           |                                     `---- reference
           |
        18 |     my_var.engine.fuel := 0;
           |            ^^^|^^
           |               `---- reference
       ----'
       ");
}

#[rstest]
fn using_references_from_using(mut with_db: RootDatabase) {
    let source1 = r#"NAMESPACE MyNs
    FUNCTION fn1 : INT
    END_FUNCTION
END_NAMESPACE
"#;

    let source2 = r#"USING MyNs;
FUNCTION fn2 : INT
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source1, source2]);
    let file2 = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();

    let offset = source2.find("MyNs").unwrap();
    let node = descendant_at(&with_db, file2, offset).unwrap();
    let refs = node.locations(&with_db).unwrap();

    assert_snapshot!(render_references(&with_db, &refs, "MyNs"), @r"
    Advice: 2 reference(s) to 'MyNs'
       ,-[ file:///test0.st:1:11 ]
       |
     1 | NAMESPACE MyNs
       |           ^^|^
       |             `--- 2 reference(s) to 'MyNs'
       |
       |-[ file:///test1.st:1:7 ]
       |
     1 | USING MyNs;
       |       ^^|^
       |         `--- reference
    ---'
    ");
}
