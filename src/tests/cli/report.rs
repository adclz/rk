use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

/// A rendered report addresses its source by character, while the spans it is
/// built from count bytes. An em-dash in a comment used to slide every
/// `declared here` forward onto a line that has nothing to do with it.
#[rstest]
fn a_label_survives_a_non_ascii_character_before_it(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : BOOL
        // — — — — — — — — — — — — — — — — — — — —
        VAR
            b : BYTE;
        END_VAR
            f := b.8;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0808] Error: multibit access out of range
       ,-[ file:///test0.st:7:18 ]
       |
     5 |             b : BYTE;
       |             ^^^^|^^^
       |                 `----- 'b' is declared here
       |
     7 |             f := b.8;
       |                  |
       |                  `-- offset 8 is out of range for type 'BYTE' (valid range: 0..7)
    ---'
    ");
}
