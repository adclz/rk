// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The control interface's own types (`rk:plc/control`), as a host on
//! wasmtime passes them to a component and gets them back: `rk test`'s, and
//! a runtime's. The names are the WIT's.

use wasmtime::component::{ComponentType, Lift, Lower};

/// `range`: a run of bytes in the module's memory, a variable's address and
/// size as the `debug-symbols` section gives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ComponentType, Lift, Lower)]
#[component(record)]
pub struct Range {
    pub address: u32,
    pub size: u32,
}

/// `scan-error`: why a scan did not run to its end.
#[derive(Debug, Clone, PartialEq, Eq, ComponentType, Lift, Lower)]
#[component(variant)]
pub enum ScanError {
    /// The message of the exception that stopped it.
    #[component(name = "raised")]
    Raised(String),
    /// No task is at that place in the `rk.schedule` section.
    #[component(name = "no-such-task")]
    NoSuchTask,
}

/// `memory-error`: why a host's read or write of the memory was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ComponentType, Lift, Lower)]
#[component(variant)]
pub enum MemoryError {
    /// The range at that place in the list is outside the memory.
    #[component(name = "outside-memory")]
    OutsideMemory(u32),
    /// The ranges together are more than the memory can hold.
    #[component(name = "too-large")]
    TooLarge,
    /// The bytes would not all land in the variables.
    #[component(name = "outside-variables")]
    OutsideVariables,
}

/// `test-state`: where a test stands after a call of `run-test`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ComponentType, Lift, Lower)]
#[component(enum)]
#[repr(u8)]
pub enum TestState {
    /// It passed: a test FUNCTION when it returns, a test FUNCTION_BLOCK
    /// when a cycle leaves its `DONE` TRUE.
    #[component(name = "passed")]
    Passed,
    /// A test FUNCTION_BLOCK whose `DONE` is still FALSE: the next call runs
    /// its next cycle.
    #[component(name = "running")]
    Running,
}

/// `test-error`: why a test did not pass.
#[derive(Debug, Clone, PartialEq, Eq, ComponentType, Lift, Lower)]
#[component(variant)]
pub enum TestError {
    /// The message it failed with.
    #[component(name = "failed")]
    Failed(String),
    /// No test is at that place in the `test-manifest` section.
    #[component(name = "no-such-test")]
    NoSuchTest,
}
