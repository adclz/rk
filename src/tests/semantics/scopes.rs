use auto_lsp::{
    default::db::{BaseDatabase, FileManager, file::File},
    lsp_types,
};

use hir::{
    HasName, HirNodeInfo,
    check::diagnostics_for_file,
    hir_def::{
        pous::pou::Pou,
        semantic_index::{get_scope, semantic_index},
        using::Using,
    },
    hir_ty::{head::signature::infer_signature, index_graphs::namespace_index, ty::Type},
};
use ide_proto::{hir_node::HirNode, walk::WalkHir};

use crate::tests::utils::find_namespace_with_name;
use crate::tests::utils::pou_name_res_from_scope;
use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;
use crate::tests::utils::{add_sources, find_pou_with_name};
use db::{RootDatabase, WorkspaceDataBase};
use std::ops::ControlFlow;

use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::semantic_index::SemanticIndex;
use insta::assert_snapshot;
use rstest::rstest;

#[test]
fn diagnostics_on_empty_file() {
    // Test that we don't panic when trying to get diagnostics for an empty file, which doesn't have a global scope.
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#""#;
    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    diagnostics_for_file(&db, file);
}

#[test]
fn global_scope() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
    FUNCTION fn1
    END_FUNCTION

    FUNCTION_BLOCK fn2
    END_FUNCTION_BLOCK

    CLASS cl
    END_CLASS

    INTERFACE in
    END_INTERFACE
"#;
    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let sema = semantic_index(&db, file);

    sema.global_pous.iter().for_each(|pou| match pou {
        Pou::Function(f) => {
            assert_eq!(pou.get_name_ident(&db).text(&db), "fn1");
            let scope = get_scope(&db, f.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::FunctionBlock(fb) => {
            assert_eq!(pou.get_name_ident(&db).text(&db), "fn2");
            let scope = get_scope(&db, fb.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::Class(c) => {
            assert_eq!(pou.get_name_ident(&db).text(&db), "cl");
            let scope = get_scope(&db, c.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::Interface(i) => {
            assert_eq!(pou.get_name_ident(&db).text(&db), "in");
            let scope = get_scope(&db, i.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::DataType(_) => {}
    });
}

#[test]
fn scoped_pous() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
NAMESPACE ns

    FUNCTION fn1
    END_FUNCTION

    FUNCTION_BLOCK fn2
    END_FUNCTION_BLOCK

    CLASS cl
    END_CLASS

    INTERFACE in
    END_INTERFACE

END_NAMESPACE
"#;
    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let sema = semantic_index(&db, file);
    let main_ns = sema.namespaces.first().unwrap();

    // Main namespaces should have Global scope as parent
    let main_ns_scope = get_scope(&db, main_ns.scope_id(&db));
    assert!(main_ns_scope.parent.is_some_and(|s| s.is_global(&db)));

    for pou in main_ns.pous(&db) {
        match pou {
            Pou::Function(f) => {
                assert_eq!(pou.get_name_ident(&db).text(&db), "fn1");
                let scope = get_scope(&db, f.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::FunctionBlock(fb) => {
                assert_eq!(pou.get_name_ident(&db).text(&db), "fn2");
                let scope = get_scope(&db, fb.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::Class(c) => {
                assert_eq!(pou.get_name_ident(&db).text(&db), "cl");
                let scope = get_scope(&db, c.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::Interface(i) => {
                assert_eq!(pou.get_name_ident(&db).text(&db), "in");
                let scope = get_scope(&db, i.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::DataType(_) => {}
        }
    }
}

#[test]
fn using_directives() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
NAMESPACE ns
    USING ns2
    USING ns3.nss
    FUNCTION fn1
    END_FUNCTION
END_NAMESPACE
"#;
    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();
    let file = db.get_file(&url).unwrap();
    let sema = semantic_index(&db, file);

    let main_ns = sema.namespaces.first().unwrap();
    let scope = get_scope(&db, main_ns.scope_id(&db));

    assert_eq!(scope.usings.len(), 2);
}

/// Utility to collect all using in a given source file.
fn collect_usings(db: &dyn WorkspaceDataBase, sema: &SemanticIndex) -> String {
    let mut result: Vec<Using> = vec![];
    let _ = sema.walk_hir(db, &mut |n| {
        if let HirNode::Using(path) = n {
            result.push(path);
        }
        ControlFlow::Continue(())
    });

    result
        .iter()
        .map(|r| {
            format!(
                "USING {}:  {}\n",
                r.path(db).to_string(db),
                namespace_index(db, *r.path(db))
                    .iter()
                    .flat_map(|ns| ns
                        .pous(db)
                        .iter()
                        .map(|pou| pou.get_name_ident(db).text(db).to_string())
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}

#[rstest]
fn child_pous_import_with_using(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    USING ns1.ns2; // bring ns2 into scope

    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns2
        FUNCTION_BLOCK fb3

        END_FUNCTION_BLOCK
    END_NAMESPACE
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // brings ns2 into scope
    assert_snapshot!(collect_usings(&with_db, sema), @"USING ns1.ns2:  fb3")
}

#[rstest]
fn sibling_pous_import_with_using(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    USING ns2;
END_NAMESPACE

NAMESPACE ns2
    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    // brings ns2 into scope
    assert_snapshot!(collect_usings(&with_db, sema), @"USING ns2:  fb3")
}

#[rstest]
fn fb3_in_scope(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    USING ns2;

    FUNCTION_BLOCK fb1
        VAR_INPUT
            i1 : fb2; // should be imported via USING ns2;
        END_VAR

    END_FUNCTION_BLOCK
END_NAMESPACE

NAMESPACE ns2
    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    assert_snapshot!(collect_usings(&with_db, sema), @r"
    USING ns2:  fb2
    fb3
    ")
}

// Global pous should be in scope in all namespaces.
#[rstest]
fn inherit_global_pous(mut with_db: RootDatabase) {
    let source1 = r#"
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns1
        FUNCTION_BLOCK fb3

        END_FUNCTION_BLOCK
    END_NAMESPACE"#;
    add_sources(&mut with_db, &[source1]);

    let file = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
        .unwrap();

    let ns1 = find_namespace_with_name(&with_db, *file, "ns1").unwrap();
    let fb1 = pou_name_res_from_scope(&with_db, ns1, "fb1").unwrap();
    let fb2 = pou_name_res_from_scope(&with_db, ns1, "fb2").unwrap();
    let fb3 = pou_name_res_from_scope(&with_db, ns1, "fb3").unwrap();

    assert_eq!(
        get_scope(&with_db, fb1.get_scope_id(&with_db))
            .parent
            .unwrap()
            .scope(&with_db),
        usize::MAX // global scope
    );

    assert_eq!(
        get_scope(&with_db, fb2.get_scope_id(&with_db))
            .parent
            .unwrap()
            .scope(&with_db),
        usize::MAX // global scope
    );

    assert_eq!(
        get_scope(&with_db, fb3.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );
}

// Both ns1 namespaces should be allowed to coexist, as they are merged.
// Therefore fb1, fb2, and fb3 should all be in scope in both namespaces.
#[rstest]
fn shared_namespaces(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    let source2 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1, source2]);

    let file1 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
        .unwrap();

    let file2 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test1.st")
        .unwrap();

    let file_0_ns1 = find_namespace_with_name(&with_db, *file1, "ns1").unwrap();
    let file_1_ns1 = find_namespace_with_name(&with_db, *file2, "ns1").unwrap();

    let fb1 = pou_name_res_from_scope(&with_db, file_0_ns1, "fb1").unwrap();
    let fb2 = pou_name_res_from_scope(&with_db, file_0_ns1, "fb2").unwrap();
    let fb3 = pou_name_res_from_scope(&with_db, file_0_ns1, "fb3").unwrap();

    assert_eq!(
        get_scope(&with_db, fb1.get_scope_id(&with_db)).parent,
        Some(file_0_ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    assert_eq!(
        get_scope(&with_db, fb2.get_scope_id(&with_db)).parent,
        Some(file_0_ns1.scope_id(&with_db)) // ns1 namespace scope
    );
    assert_eq!(
        get_scope(&with_db, fb3.get_scope_id(&with_db)).parent,
        Some(file_1_ns1.scope_id(&with_db)) // ns1 namespace scope (other file)
    );
}

// ns1 and ns2 are separate namespaces, so fb3 should not be in scope in ns1.
#[rstest]
fn unshared_namespaces(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    let source2 = r#"
NAMESPACE ns2
    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1, source2]);

    let file1 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
        .unwrap();

    let file2 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test1.st")
        .unwrap();

    let ns1 = find_namespace_with_name(&with_db, *file1, "ns1").unwrap();
    let ns2 = find_namespace_with_name(&with_db, *file2, "ns2").unwrap();

    let fb1 = pou_name_res_from_scope(&with_db, ns1, "fb1").unwrap();
    let fb2 = pou_name_res_from_scope(&with_db, ns1, "fb2").unwrap();
    let fb3 = pou_name_res_from_scope(&with_db, ns2, "fb3").unwrap();

    assert_eq!(
        get_scope(&with_db, fb1.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    assert_eq!(
        get_scope(&with_db, fb2.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    assert_eq!(
        get_scope(&with_db, fb3.get_scope_id(&with_db)).parent,
        Some(ns2.scope_id(&with_db)) // ns1 namespace scope
    );
}

// ns1 should have access to ns2's pous because of the USING statement.
#[rstest]
fn shared_namespaces_with_using(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    USING ns2; // bring ns2 into scope

    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    let source2 = r#"
NAMESPACE ns2
    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1, source2]);

    let file1 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
        .unwrap();

    let file2 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test1.st")
        .unwrap();

    let ns1 = find_namespace_with_name(&with_db, *file1, "ns1").unwrap();
    let ns2 = find_namespace_with_name(&with_db, *file2, "ns2").unwrap();

    let fb1 = pou_name_res_from_scope(&with_db, ns1, "fb1").unwrap();
    let fb2 = pou_name_res_from_scope(&with_db, ns1, "fb2").unwrap();
    let fb3 = pou_name_res_from_scope(&with_db, ns1, "fb3").unwrap();

    assert_eq!(
        get_scope(&with_db, fb1.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    assert_eq!(
        get_scope(&with_db, fb2.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    // fb3 is in file2
    assert_eq!(
        get_scope(&with_db, fb3.get_scope_id(&with_db)).parent,
        Some(ns2.scope_id(&with_db)) // ns2 namespace scope (but available in ns1 due to USING)
    );
}

// Pous declared in parent namespace should be in scope in child namespace.
// Pous from ns1 should be in scope in ns2 because ns2 is nested inside ns1.
#[rstest]
fn parent_pous_inherit(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns2
        FUNCTION_BLOCK fb3

        END_FUNCTION_BLOCK
    END_NAMESPACE
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1]);

    let file1 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
        .unwrap();

    let ns1 = find_namespace_with_name(&with_db, *file1, "ns1").unwrap();
    let ns2 = find_namespace_with_name(&with_db, *file1, "ns1.ns2").unwrap();

    let fb1 = pou_name_res_from_scope(&with_db, ns2, "fb1").unwrap();
    let fb2 = pou_name_res_from_scope(&with_db, ns2, "fb2").unwrap();
    let fb3 = pou_name_res_from_scope(&with_db, ns2, "fb3").unwrap();

    assert_eq!(
        get_scope(&with_db, fb1.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    assert_eq!(
        get_scope(&with_db, fb2.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    assert_eq!(
        get_scope(&with_db, fb3.get_scope_id(&with_db)).parent,
        Some(ns2.scope_id(&with_db)) // ns2 namespace scope (but available in ns1 due to nesting)
    );
}

// However parent namespaces should not have access to child namespace pous.
// ns1 namespace should not have access to ns2's pous.
#[rstest]
fn child_pous_inherit(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns2
        FUNCTION_BLOCK fb3

        END_FUNCTION_BLOCK
    END_NAMESPACE
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1]);

    let file1 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
        .unwrap();

    let ns1 = find_namespace_with_name(&with_db, *file1, "ns1").unwrap();
    let fb1 = pou_name_res_from_scope(&with_db, ns1, "fb1").unwrap();
    let fb2 = pou_name_res_from_scope(&with_db, ns1, "fb2").unwrap();
    let fb3 = pou_name_res_from_scope(&with_db, ns1, "fb3");

    assert_eq!(
        get_scope(&with_db, fb1.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    assert_eq!(
        get_scope(&with_db, fb2.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    assert!(fb3.is_none());
}

// A Using directive in a parent namespace should bring pous from the used namespace,
// even in child namespaces.
#[rstest]
fn child_pous_inherit_with_using(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    USING ns1.ns2; // bring ns2 into scope

    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns2
        FUNCTION_BLOCK fb3

        END_FUNCTION_BLOCK
    END_NAMESPACE
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1]);

    let ns1 = find_namespace_with_name(
        &with_db,
        *with_db
            .get_files()
            .iter()
            .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
            .unwrap(),
        "ns1",
    )
    .unwrap();

    let ns2 = find_namespace_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "ns1.ns2",
    )
    .unwrap();

    let fb1 = pou_name_res_from_scope(&with_db, ns2, "fb1").unwrap();
    let fb2 = pou_name_res_from_scope(&with_db, ns2, "fb2").unwrap();
    let fb3 = pou_name_res_from_scope(&with_db, ns2, "fb3").unwrap();

    assert_eq!(
        get_scope(&with_db, fb1.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );

    assert_eq!(
        get_scope(&with_db, fb2.get_scope_id(&with_db)).parent,
        Some(ns1.scope_id(&with_db)) // ns1 namespace scope
    );
    assert_eq!(
        get_scope(&with_db, fb3.get_scope_id(&with_db)).parent,
        Some(ns2.scope_id(&with_db)) // ns2 namespace scope (but available in ns1 due to USING)
    );
}

#[rstest]
fn unknown_namespace_in_using(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    USING unknown_ns; // unknown namespace
END_NAMESPACE

FUNCTION_BLOCK fb1
    USING unknown_ns2; // unknown namespace
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0216] Error: namespace not found
       ,-[ file:///test0.st:3:11 ]
       |
     3 |     USING unknown_ns; // unknown namespace
       |           ^^^^^|^^^^
       |                `------ namespace 'unknown_ns' not found
    ---'
    [E0216] Error: namespace not found
       ,-[ file:///test0.st:7:11 ]
       |
     7 |     USING unknown_ns2; // unknown namespace
       |           ^^^^^|^^^^^
       |                `------- namespace 'unknown_ns2' not found
    ---'
    ");
}

// usage of fully qualified paths in variable type
#[rstest]
fn fully_qualified_path_in_var_type(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns
	FUNCTION_BLOCK fb1

	END_FUNCTION_BLOCK
END_NAMESPACE

FUNCTION_BLOCK fb
    VAR_INPUT
        i1 : ns.fb1; // should resolve to fb1 in ns namespace
    END_VAR

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source1]);

    let file1 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
        .unwrap();

    let fb = find_pou_with_name(&with_db, *file1, "fb").unwrap();
    let variables = &fb.get_scope_id(&with_db).def_map(&with_db).global_variables;
    let var_i1 = variables.get(&Ident::from_slice(&with_db, "i1")).unwrap();
    let infer = infer_signature(&with_db, var_i1.scope_id(&with_db));
    let ty = infer.type_of_specs[&var_i1.spec(&with_db)];

    assert!(matches!(ty, Type::FunctionBlock(_)));
}

// usage of fully qualified paths in EXTENDS clause
#[rstest]
fn fully_qualified_path_in_extends(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns
	CLASS cl1

	END_CLASS
END_NAMESPACE

FUNCTION_BLOCK fb EXTENDS ns.cl1

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source1]);

    let file1 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
        .unwrap();

    let fb = find_pou_with_name(&with_db, *file1, "fb").unwrap();

    let inehrited = fb.get_scope_id(&with_db).inheritors(&with_db);
    assert_eq!(inehrited.len(), 1);
    assert_eq!(
        inehrited
            .values().next()
            .unwrap()
            .get_name_ident(&with_db)
            .text(&with_db),
        "cl1"
    );
}

// usage of fully qualified paths in IMPLEMENTS clause
#[rstest]
fn fully_qualified_path_in_implements(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns
	INTERFACE in1

	END_INTERFACE
END_NAMESPACE

FUNCTION_BLOCK fb IMPLEMENTS ns.in1

END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source1]);

    let file1 = with_db
        .get_files()
        .iter()
        .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
        .unwrap();

    let fb = find_pou_with_name(&with_db, *file1, "fb").unwrap();

    let inehrited = fb.get_scope_id(&with_db).inheritors(&with_db);
    assert_eq!(inehrited.len(), 1);
    assert_eq!(
        inehrited
            .values().next()
            .unwrap()
            .get_name_ident(&with_db)
            .text(&with_db),
        "in1"
    );
}
