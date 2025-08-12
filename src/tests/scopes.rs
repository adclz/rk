use auto_lsp::{
    default::db::{file::File, BaseDatabase, FileManager},
    lsp_types,
};

use db::{
    hir::{
        pous::pou::Pou,
        scopes::scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
    RootDatabase,
};

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

    // Global scope should have no parent
    assert_eq!(sema.get_scope(FileScopeId::global(file)).parent, None);

    sema.global_pous.iter().for_each(|pou| match pou.pou(&db) {
        Pou::Function(f) => {
            assert_eq!(pou.name(&db).text(&db), "fn1");
            assert!(f.scope_id(&db).is_global());
        }
        Pou::FunctionBlock(fb) => {
            assert_eq!(pou.name(&db).text(&db), "fn2");
            assert!(fb.scope_id(&db).is_global());
        }
        Pou::Class(c) => {
            assert_eq!(pou.name(&db).text(&db), "cl");
            assert!(c.scope_id(&db).is_global());
        }
        Pou::Interface(i) => {
            assert_eq!(pou.name(&db).text(&db), "in");
            assert!(i.scope_id(&db).is_global());
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
    let scope = sema.get_scope(main_ns.scope_id(&db));
    assert_eq!(scope.parent, Some(FileScopeId::global(file)));

    let scope = sema
        .scopes
        .iter()
        .find_map(|(_id, scope)| {
            if let ScopeKind::Namespace(_) = scope.kind {
                Some(scope)
            } else {
                None
            }
        })
        .unwrap();

    for pou in main_ns.pous(&db) {
        match pou.pou(&db) {
            Pou::Function(f) => {
                assert_eq!(pou.name(&db).text(&db), "fn1");
                assert_eq!(f.scope_id(&db), scope.id);
            }
            Pou::FunctionBlock(fb) => {
                assert_eq!(pou.name(&db).text(&db), "fn2");
                assert_eq!(fb.scope_id(&db), scope.id);
            }
            Pou::Class(c) => {
                assert_eq!(pou.name(&db).text(&db), "cl");
                assert_eq!(c.scope_id(&db), scope.id);
            }
            Pou::Interface(i) => {
                assert_eq!(pou.name(&db).text(&db), "in");
                assert_eq!(i.scope_id(&db), scope.id);
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
    let scope = sema.get_scope(main_ns.scope_id(&db));

    assert_eq!(scope.usings.len(), 2);
}
