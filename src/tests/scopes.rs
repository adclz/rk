use auto_lsp::{
    default::db::{BaseDatabase, FileManager, file::File},
    lsp_types,
};

use hir::def::{pous::pou::Pou, scope::FileScopeId, semantic_index::semantic_index};

use db::RootDatabase;

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

    sema.global_pous.iter().for_each(|pou| match pou.pou(&db) {
        Pou::Function(f) => {
            assert_eq!(pou.name(&db).text(&db), "fn1");
            let scope = sema.get_scope(&db, f.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::FunctionBlock(fb) => {
            assert_eq!(pou.name(&db).text(&db), "fn2");
            let scope = sema.get_scope(&db, fb.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::Class(c) => {
            assert_eq!(pou.name(&db).text(&db), "cl");
            let scope = sema.get_scope(&db,c.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::Interface(i) => {
            assert_eq!(pou.name(&db).text(&db), "in");
            let scope = sema.get_scope(&db,i.scope_id(&db));
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
    let main_ns_scope = sema.get_scope(&db, main_ns.scope_id(&db));
    assert!(main_ns_scope.parent.is_some_and(|s| s.is_global(&db)));

    for pou in main_ns.pous(&db) {
        match pou.pou(&db) {
            Pou::Function(f) => {
                assert_eq!(pou.name(&db).text(&db), "fn1");
                let scope = sema.get_scope(&db, f.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::FunctionBlock(fb) => {
                assert_eq!(pou.name(&db).text(&db), "fn2");
                let scope = sema.get_scope(&db, fb.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::Class(c) => {
                assert_eq!(pou.name(&db).text(&db), "cl");
                let scope = sema.get_scope(&db, c.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::Interface(i) => {
                assert_eq!(pou.name(&db).text(&db), "in");
                let scope = sema.get_scope(&db, i.scope_id(&db));
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
    let scope = sema.get_scope(&db, main_ns.scope_id(&db));

    assert_eq!(scope.usings.len(), 2);
}
