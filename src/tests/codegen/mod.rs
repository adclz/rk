//! Test modules for WASM codegen; the helpers live in [`harness`].

// Test modules - only execution tests, no validation-only tests
mod aggregate_returns;
mod arrays;
mod bit_access;
mod classes;
mod control_flow;
mod debug_functions;
mod debug_lines;
mod debug_stacktrace;
mod debug_symbols;
mod e2e;
mod empty_bodies;
mod enums;
mod exceptions_spike;
mod execution;
mod time_literals;
mod expt;
mod fb_dispatch;
mod function_blocks;
mod function_inputs;
mod function_outputs;
mod globals;
mod modulo;
mod imports;
mod initializers;
mod inout;
mod locals;
mod located;
mod instance_initializers;
mod mir_smoke;
mod namespaces;
mod overloads;
mod profile_swap;
mod ref_to;
mod references;
mod schedule;
mod strings;
mod structs;
mod traps;
mod unary_ops;
mod var_config;
mod variadics;
mod wasm_pragma;

pub use crate::tests::utils::with_db;

pub use crate::tests::utils::add_source;

mod harness;
pub use harness::*;
