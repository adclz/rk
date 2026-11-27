// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What the tests that run `rk` share: the cache its builds land in.

use std::path::{Path, PathBuf};

/// The cache the tests share, the in-process ones too, as builds share a
/// user's: the standard library and the optimizer are written there once.
pub fn tests_cache() -> PathBuf {
    std::env::temp_dir().join("rk-tests-cache")
}

/// The build folder of a workspace that exists, in [`tests_cache`],
/// deleted when the guard is, so that no test leaves one behind.
pub struct Builds(PathBuf);

impl Builds {
    pub fn of(ws: &Path) -> Self {
        Builds(db::cache::build_dir(&tests_cache(), ws))
    }

    pub fn dir(&self) -> &Path {
        &self.0
    }
}

impl Drop for Builds {
    fn drop(&mut self) {
        let _ = db::cache::remove(&self.0);
    }
}
