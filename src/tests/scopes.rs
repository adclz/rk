use auto_lsp::{
    default::db::{file::File, BaseDatabase, FileManager},
    lsp_types,
};

use db::{
    hir::{pous::pou::Pou, scopes::scope::ScopeId, semantic_index::semantic_index},
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

    sema.pou_keys
        .iter()
        .for_each(|(key, pou)| match pou.pou(&db) {
            Pou::Function(f) => {
                assert_eq!(pou.name(&db).text(&db), "fn1");
                assert_eq!(f.scope_id(&db), ScopeId::global());
            }
            Pou::FunctionBlock(fb) => {
                assert_eq!(pou.name(&db).text(&db), "fn2");
                assert_eq!(fb.scope_id(&db), ScopeId::global());
            }
            Pou::Class(c) => {
                assert_eq!(pou.name(&db).text(&db), "cl");
                assert_eq!(c.scope_id(&db), ScopeId::global());
            }
            Pou::Interface(i) => {
                assert_eq!(pou.name(&db).text(&db), "in");
                assert_eq!(i.scope_id(&db), ScopeId::global());
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

    let main_ns = sema.namespace_keys.values().next().unwrap();
    let scope = sema.get_scope(main_ns.scope_id(&db));

    for pou in sema.pou_keys.values() {
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
fn scoped_nested_pous() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
NAMESPACE ns

    NAMESPACE nss2
        FUNCTION fn1
        END_FUNCTION

        FUNCTION_BLOCK fn2
        END_FUNCTION_BLOCK

        CLASS cl
        END_CLASS

        INTERFACE in
        END_INTERFACE

    END_NAMESPACE

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

    let nested_ns = sema
        .namespace_keys
        .iter()
        .find(|(k, n)| n.path(&db).to_string(&db) == "ns.nss2")
        .unwrap()
        .1;

    let scope = sema.get_scope(nested_ns.scope_id(&db));

    for pou in sema.pou_keys.values() {
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

    let main_ns = sema.namespace_keys.values().next().unwrap();
    let scope = sema.get_scope(main_ns.scope_id(&db));

    assert_eq!(scope.usings.len(), 2);
}
