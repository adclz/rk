//! The call graph HIR keeps for recursion: which bodies reach themselves
//! again, following dispatch the way lowering does.

use auto_lsp::default::db::file::File;
use db::{RootDatabase, WorkspaceDataBase};
use hir::{
    HirNodeInfo,
    hir_def::{pous::pou::Pou, semantic_index::semantic_index},
    hir_ty::{
        calls::{CallNode, is_recursive},
        oop::MethodRef,
    },
};
use rstest::rstest;

use crate::tests::utils::{add_source, with_db};

/// Every body declared in `file`: FUNCTIONs, FB bodies and METHODs.
fn bodies<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> Vec<CallNode<'db>> {
    let mut bodies = Vec::new();
    for pou in semantic_index(db, file).global_pous.iter() {
        match pou {
            Pou::Function(f) => bodies.push(CallNode::Function(*f)),
            Pou::FunctionBlock(fb) => bodies.push(CallNode::Body(*fb)),
            _ => {}
        }
        for method in pou.get_scope_id(db).def_map(db).declared_methods.values() {
            if let MethodRef::Declared(decl) = method {
                bodies.push(CallNode::Method(*decl));
            }
        }
    }
    bodies
}

/// The bodies of `source` that may call themselves, sorted by name.
fn recursive(db: &mut RootDatabase, source: &str) -> Vec<String> {
    let file = add_source(db, source);
    let mut names: Vec<String> = bodies(db, file)
        .into_iter()
        .filter(|node| is_recursive(db, *node))
        .map(|node| node.display_name(db))
        .collect();
    names.sort();
    names
}

#[rstest]
fn direct_and_mutual_recursion(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION SumTo : INT
VAR_INPUT n : INT; END_VAR
    IF n > 0 THEN SumTo := n + SumTo(n - 1); END_IF;
END_FUNCTION

FUNCTION Ping : INT
VAR_INPUT n : INT; END_VAR
    IF n > 0 THEN Ping := Pong(n - 1); END_IF;
END_FUNCTION

FUNCTION Pong : INT
VAR_INPUT n : INT; END_VAR
    Pong := Ping(n);
END_FUNCTION

FUNCTION Caller : INT
    Caller := SumTo(3) + Ping(2);
END_FUNCTION
"#;
    assert_eq!(recursive(&mut with_db, source), ["Ping", "Pong", "SumTo"]);
}

/// A syntax error elsewhere in the file leaves the recursion found. Salsa
/// refuses a cycle whose inputs accumulated anything, and the parse errors
/// were accumulated: this panicked.
#[rstest]
fn recursion_beside_a_syntax_error(mut with_db: RootDatabase) {
    let source = r#"
@@@ garbage ###

FUNCTION SumTo : INT
VAR_INPUT n : INT; END_VAR
    IF n > 0 THEN SumTo := n + SumTo(n - 1); END_IF;
END_FUNCTION
"#;
    assert_eq!(recursive(&mut with_db, source), ["SumTo"]);
}

/// A call in a local's initializer runs at every call too.
#[rstest]
fn a_call_in_an_initializer(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Seed : INT
VAR s : INT := Grow(1); END_VAR
    Seed := s;
END_FUNCTION

FUNCTION Grow : INT
VAR_INPUT n : INT; END_VAR
    IF n > 0 THEN Grow := Seed(); END_IF;
END_FUNCTION
"#;
    assert_eq!(recursive(&mut with_db, source), ["Grow", "Seed"]);
}

/// `THIS.Hook()` runs the instance's override, so a base method reaches
/// every override of what it calls.
#[rstest]
fn this_reaches_the_overrides(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Base
    METHOD PUBLIC Run : INT
        Run := THIS.Hook();
    END_METHOD
    METHOD PUBLIC Hook : INT
        Hook := 0;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
    METHOD PUBLIC OVERRIDE Hook : INT
        Hook := Run();
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_eq!(
        recursive(&mut with_db, source),
        ["Base.Run", "Derived.Hook"]
    );
}

/// A call on an instance runs the method of the instance's own type: no
/// override further down is reached through it.
#[rstest]
fn a_call_on_an_instance_is_static(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK B2
    METHOD PUBLIC M : INT
        M := 1;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK D2 EXTENDS B2
    METHOD PUBLIC OVERRIDE M : INT
        M := UseBase();
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION UseBase : INT
VAR b : B2; END_VAR
    UseBase := b.M();
END_FUNCTION
"#;
    assert!(recursive(&mut with_db, source).is_empty());
}

/// A call through an INTERFACE reaches every implementer, and the POUs
/// deriving from one.
#[rstest]
fn an_interface_reaches_every_implementer(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE IShow
    METHOD Show : INT END_METHOD
END_INTERFACE

FUNCTION Ask : INT
VAR_INPUT dev : IShow; END_VAR
    Ask := dev.Show();
END_FUNCTION

FUNCTION_BLOCK Plain IMPLEMENTS IShow
    METHOD PUBLIC Show : INT
        Show := 1;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Chatty EXTENDS Plain
    METHOD PUBLIC OVERRIDE Show : INT
        Show := Ask(dev := THIS^);
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_eq!(recursive(&mut with_db, source), ["Ask", "Chatty.Show"]);
}

/// `SUPER()` runs the base's body.
#[rstest]
fn super_runs_the_base_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Base
    Kick();
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
    SUPER();
END_FUNCTION_BLOCK

FUNCTION Kick : INT
VAR d : Derived; END_VAR
    d();
END_FUNCTION
"#;
    assert_eq!(recursive(&mut with_db, source), ["Base", "Derived", "Kick"]);
}

/// An edit that breaks the cycle takes the recursion away: the settled
/// cycle is no memo that outlives its cause.
#[rstest]
fn breaking_the_cycle_ends_the_recursion(mut with_db: RootDatabase) {
    use auto_lsp::lsp_types::{
        DidChangeTextDocumentParams, Position, Range, TextDocumentContentChangeEvent,
        VersionedTextDocumentIdentifier,
    };
    let source = "FUNCTION Ping : INT\n    Ping := Pong();\nEND_FUNCTION\n\nFUNCTION Pong : INT\n    Pong := Ping();\nEND_FUNCTION\n";
    let file = add_source(&mut with_db, source);
    let recursive = |db: &RootDatabase| -> Vec<bool> {
        bodies(db, file)
            .into_iter()
            .map(|node| is_recursive(db, node))
            .collect()
    };
    assert_eq!(recursive(&with_db), [true, true]);

    // `Pong := Ping();` becomes `Pong := 1;`.
    let event = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: file.url(&with_db).clone(),
            version: 1,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(5, 12), Position::new(5, 18))),
            range_length: None,
            text: "1".into(),
        }],
    };
    file.update_edit(&mut with_db, &event).unwrap();
    assert!(file.document(&with_db).texter.text.contains("Pong := 1;"));
    assert_eq!(recursive(&with_db), [false, false]);
}

/// Random call graphs, each body calling some others: a body is recursive
/// exactly when it reaches itself, whichever body is asked first. Salsa
/// settles a cycle from the body it was entered from, so cycles sharing
/// bodies, entered in every order, are the case to cover.
#[rstest]
#[case::small(4, 0.35)]
#[case::medium(8, 0.25)]
#[case::dense(10, 0.4)]
fn random_graphs_match_reachability(#[case] size: usize, #[case] density: f64) {
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d ^ (size as u64 * 7919);
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..25 {
        let edges: Vec<Vec<usize>> = (0..size)
            .map(|_| {
                (0..size)
                    .filter(|_| (next() % 1000) as f64 / 1000.0 < density)
                    .collect()
            })
            .collect();
        let source: String = edges
            .iter()
            .enumerate()
            .map(|(i, callees)| {
                let calls: String = callees.iter().map(|c| format!("    f{c}();\n")).collect();
                format!("FUNCTION f{i} : INT\n{calls}END_FUNCTION\n\n")
            })
            .collect();

        // Each body's own reachability, the plain way.
        let expected: Vec<bool> = (0..size)
            .map(|start| {
                let mut seen = vec![false; size];
                let mut pending = edges[start].clone();
                while let Some(node) = pending.pop() {
                    if !seen[node] {
                        seen[node] = true;
                        pending.extend(&edges[node]);
                    }
                }
                seen[start]
            })
            .collect();

        // Asked in a rotating order, in a fresh database each time.
        for first in 0..size {
            let mut db = RootDatabase::default();
            let file = add_source(&mut db, &source);
            let nodes = bodies(&db, file);
            let mut order: Vec<usize> = (0..size).collect();
            order.rotate_left(first);
            for i in order {
                let node = nodes
                    .iter()
                    .find(|node| node.display_name(&db) == format!("f{i}"))
                    .copied()
                    .unwrap();
                assert_eq!(
                    is_recursive(&db, node),
                    expected[i],
                    "f{i} asked after f{first} in\n{source}"
                );
            }
        }
    }
}
