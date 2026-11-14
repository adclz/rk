// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What ST cannot pass yet is left out, and the file says why at its end:
//! a list, a variant, flags, more values than the canonical ABI passes as
//! values. The rest of the interface is generated. A function the world
//! imports itself, outside an interface, stops the generator.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::checked;
use crate::tests::utils::with_db;

/// Each kind of WIT type ST has no type for yet, as a type of the
/// interface, taken and given; what is left of the interface is generated.
#[rstest]
fn what_st_has_no_type_for(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface leftovers {
            variant shape { circle(f32), square }
            flags mode { read, write }
            type bytes = list<u8>;
            draw: func(figure: shape);
            pick: func() -> shape;
            set: func(how: mode);
            get: func() -> mode;
            send: func(data: list<u8>);
            names: func() -> list<string>;
            kept: func(x: u32) -> u32;
        }

        world example {
            import leftovers;
        }
    "#), @r"
    // wasi-example-leftovers.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Leftovers
    			{extern 'wasi:example/leftovers@0.3.0' 'kept'}
    			FUNCTION KEPT : UDINT
    			VAR_INPUT
    				x : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `shape`: it is a variant, which ST has no type for yet.
       - `mode`: it is flags, which ST has no type for yet.
       - `bytes`: it is a list, which ST has no type for yet.
       - `draw`: its parameter `figure` holds a variant, which an import cannot
         take yet.
       - `pick`: its result holds a variant, which an import cannot give back
         yet.
       - `set`: its parameter `how` holds flags, which an import cannot take
         yet.
       - `get`: its result holds flags, which an import cannot give back yet.
       - `send`: its parameter `data` holds a list, which an import cannot take
         yet.
       - `names`: its result holds a list, which an import cannot give back
         yet. */
    ");
}

/// 16 values are passed as values, the 17th in memory, which an import
/// declared in ST cannot do: a string counts as two.
#[rstest]
fn past_16_values(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface wide {
            sixteen: func(a: u32, b: u32, c: u32, e: u32, f: u32, g: u32, h: u32, j: u32, k: u32, o: u32, t: u32, u: u32, v: u32, w: u32, x: u32, y: u32);
            seventeen: func(a: u32, b: u32, c: u32, e: u32, f: u32, g: u32, h: u32, j: u32, k: u32, o: u32, t: u32, u: u32, v: u32, w: u32, x: u32, y: u32, z: u32);
            strings: func(a: string, b: string, c: string, e: string, f: string, g: string, h: string, j: string, k: u32);
        }

        world example {
            import wide;
        }
    "#), @r"
    // wasi-example-wide.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Wide
    			{extern 'wasi:example/wide@0.3.0' 'sixteen'}
    			FUNCTION SIXTEEN
    			VAR_INPUT
    				a : UDINT;
    				b : UDINT;
    				c : UDINT;
    				e : UDINT;
    				f : UDINT;
    				g : UDINT;
    				h : UDINT;
    				j : UDINT;
    				k : UDINT;
    				o : UDINT;
    				t : UDINT;
    				u : UDINT;
    				v : UDINT;
    				w : UDINT;
    				x : UDINT;
    				y : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    /* Not generated:
       - `seventeen`: it takes 17 values, which the canonical ABI passes in
         memory past 16.
       - `strings`: it takes 17 values, which the canonical ABI passes in
         memory past 16. */
    ");
}

/// An interface with nothing ST can declare is a file that only says so.
#[rstest]
fn an_interface_with_nothing_to_declare(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface empty-handed {
            names: func() -> list<string>;
        }

        world example {
            import empty-handed;
        }
    "#), @r"
    // wasi-example-empty-handed.st
    /* Not generated:
       - `names`: its result holds a list, which an import cannot give back
         yet. */
    ");
}

/// A function the world imports itself has no interface to be named after:
/// the generator stops.
#[test]
fn a_function_of_the_world_stops_the_generator() {
    let (resolve, world) = rk_bindgen::resolve(
        r#"
        package wasi:example@0.3.0;

        world example {
            import tick: func();
        }
    "#,
    )
    .expect("the WIT resolves");
    assert_snapshot!(rk_bindgen::generate(&resolve, world).unwrap_err(), @"the world imports `tick` itself: rk imports a function from an interface");
}
