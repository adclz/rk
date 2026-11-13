// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Resources: a handle is a UDINT; a resource's constructor, methods and
//! static functions are FUNCTIONs named after it, a method taking the
//! handle it is called on first as `self`; an async method is a block; the
//! drop lets go of a handle the host gave.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::checked;
use crate::tests::utils::with_db;

/// Every kind of function a resource has, and its drop.
#[rstest]
fn a_resource_s_functions_and_its_drop(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface files {
            resource descriptor {
                constructor(mode: u32);
                size: func() -> u64;
                resize: func(to: u64) -> bool;
                open: static func(at: u32) -> descriptor;
                flush: async func();
                read-at: async func(offset: u64) -> u32;
            }
        }

        world example {
            import files;
        }
    "#), @r"
    // wasi-example-files.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Files
    			/* Lets go of `handle`, a handle of `descriptor` the host gave. */
    			{extern 'wasi:example/files@0.3.0' '[resource-drop]descriptor'}
    			FUNCTION DESCRIPTOR_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/files@0.3.0' '[constructor]descriptor'}
    			FUNCTION DESCRIPTOR_NEW : UDINT
    			VAR_INPUT
    				mode : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/files@0.3.0' '[method]descriptor.size'}
    			FUNCTION DESCRIPTOR_SIZE : ULINT
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/files@0.3.0' '[method]descriptor.resize'}
    			FUNCTION DESCRIPTOR_RESIZE : BOOL
    			VAR_INPUT
    				self : UDINT;
    				to_ : ULINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/files@0.3.0' '[static]descriptor.open'}
    			FUNCTION DESCRIPTOR_OPEN : UDINT
    			VAR_INPUT
    				at_ : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern async 'wasi:example/files@0.3.0' '[method]descriptor.flush'}
    			FUNCTION_BLOCK DESCRIPTOR_FLUSH
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK

    			{extern async 'wasi:example/files@0.3.0' '[method]descriptor.read-at'}
    			FUNCTION_BLOCK DESCRIPTOR_READ_AT
    			VAR_INPUT
    				self : UDINT;
    				offset : ULINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    				value : UDINT;
    			END_VAR
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A constructor that takes nothing, and handles owned and borrowed, of
/// this interface's resources and of another's, in and out.
#[rstest]
fn handles_go_in_and_come_back_as_udint(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface types {
            resource socket;
        }

        interface network {
            use types.{socket};
            resource pool {
                constructor();
                take: func() -> socket;
                give: func(s: socket);
                peek: func(s: borrow<socket>) -> bool;
            }
            swap: func(a: pool, b: borrow<pool>) -> pool;
        }

        world example {
            import network;
        }
    "#), @r"
    // wasi-example-types.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Types
    			/* Lets go of `handle`, a handle of `socket` the host gave. */
    			{extern 'wasi:example/types@0.3.0' '[resource-drop]socket'}
    			FUNCTION SOCKET_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE

    // wasi-example-network.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Network
    			/* Lets go of `handle`, a handle of `pool` the host gave. */
    			{extern 'wasi:example/network@0.3.0' '[resource-drop]pool'}
    			FUNCTION POOL_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/network@0.3.0' '[constructor]pool'}
    			FUNCTION POOL_NEW : UDINT
    			END_FUNCTION

    			{extern 'wasi:example/network@0.3.0' '[method]pool.take'}
    			FUNCTION POOL_TAKE : UDINT
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/network@0.3.0' '[method]pool.give'}
    			FUNCTION POOL_GIVE
    			VAR_INPUT
    				self : UDINT;
    				s_ : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/network@0.3.0' '[method]pool.peek'}
    			FUNCTION POOL_PEEK : BOOL
    			VAR_INPUT
    				self : UDINT;
    				s_ : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/network@0.3.0' 'swap'}
    			FUNCTION SWAP : UDINT
    			VAR_INPUT
    				a : UDINT;
    				b : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// Two resources with a method of one name are two FUNCTIONs, each named
/// after its resource.
#[rstest]
fn methods_of_one_name_on_two_resources(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, r#"
        package wasi:example@0.3.0;

        interface pair {
            resource input { get: func() -> u32; }
            resource output { get: func() -> u32; }
        }

        world example {
            import pair;
        }
    "#), @r"
    // wasi-example-pair.st
    NAMESPACE INTERNAL Wasi
    	NAMESPACE Example
    		NAMESPACE Pair
    			/* Lets go of `handle`, a handle of `input` the host gave. */
    			{extern 'wasi:example/pair@0.3.0' '[resource-drop]input'}
    			FUNCTION INPUT_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			/* Lets go of `handle`, a handle of `output` the host gave. */
    			{extern 'wasi:example/pair@0.3.0' '[resource-drop]output'}
    			FUNCTION OUTPUT_DROP
    			VAR_INPUT
    				handle : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/pair@0.3.0' '[method]input.get'}
    			FUNCTION INPUT_GET : UDINT
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			END_FUNCTION

    			{extern 'wasi:example/pair@0.3.0' '[method]output.get'}
    			FUNCTION OUTPUT_GET : UDINT
    			VAR_INPUT
    				self : UDINT;
    			END_VAR
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}
