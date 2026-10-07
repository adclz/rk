// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

pub mod rules;

pub use rules::ALL_RULE_NAMES;
pub use rules::STYLE_RULE_NAMES;
pub use rules::is_enabled;
pub use rules::is_recommended;
pub use rules::lint_file;
