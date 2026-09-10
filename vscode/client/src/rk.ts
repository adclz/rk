//! Which `rk` CLI the extension runs.
//!
//! One answer for the whole extension: the debugger, the standard-library
//! probe, the build task and the test task all ask here, so they can never
//! disagree about which build of the toolchain the editor is speaking to.

import { ExtensionContext, ExtensionMode, workspace } from "vscode";
import { existsSync } from "fs";
import { join } from "path";

let context: ExtensionContext | undefined;

/// Called once at activation: how [`developmentRk`] knows where the checkout
/// is and whether this is a development host at all.
export function rememberExtension(ctx: ExtensionContext) {
  context = ctx;
}

/// The `rk` CLI command: the `rk.path` setting if the user set one, else
/// the checkout's own build in a development host, else `rk` from PATH
/// (rust-analyzer's configurable-binary pattern).
///
/// The middle rule earns its place. `rk` is BOTH the compiler that writes a
/// program's debug symbols and the adapter that reads them back, so a stale one
/// on PATH is stale on both sides at once and the mismatch cancels out: it
/// debugs an old program happily, showing another build's answers. The language
/// server cannot go stale this way because the F5 task rebuilds and copies it;
/// this gives `rk` the same link.
export function rkCommand(): string {
  // `get` would answer with package.json's default, which is indistinguishable
  // from a user who typed `rk`; only an explicit setting may outrank the
  // checkout.
  const setting = workspace.getConfiguration("rk").inspect<string>("path");
  const explicit =
    setting?.workspaceFolderValue ?? setting?.workspaceValue ?? setting?.globalValue;
  if (explicit) {
    return explicit;
  }
  return developmentRk() ?? "rk";
}

/// The `rk` this checkout builds, when the extension is being run FROM it
/// (`--extensionDevelopmentPath`), and it exists. Never in a packaged
/// extension: there is no checkout, and PATH is the answer.
function developmentRk(): string | undefined {
  if (context?.extensionMode !== ExtensionMode.Development) {
    return undefined;
  }
  const binary = process.platform === "win32" ? "rk.exe" : "rk";
  // `<checkout>/vscode` is the extension root, so the target dir is one up.
  const built = join(context.extensionPath, "..", "target", "release", binary);
  return existsSync(built) ? built : undefined;
}
