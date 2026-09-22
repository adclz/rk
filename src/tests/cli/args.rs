use rk::cli::Args;

use clap::Parser;

/// The VSCode extension's command lines must parse. Read from the
/// extension's source rather than restated, so a change to the argv in
/// TypeScript is checked here.
#[test]
fn the_vscode_extension_invokes_a_cli_that_exists() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/vscode/client/src/extension.ts"
    ))
    .expect("the extension source is part of this repo");

    // `rkCommand(), ["compile", "--workspace", ws]` — the argv arrays
    // handed to ProcessExecution and execFileSync.
    let invocations: Vec<Vec<String>> = source
        .match_indices("rkCommand(), [")
        .map(|(i, m)| {
            let rest = &source[i + m.len()..];
            let end = rest.find(']').expect("an argv array is closed");
            rest[..end]
                .split(',')
                .map(|a| a.trim().trim_matches('"').to_string())
                .filter(|a| !a.is_empty())
                .collect()
        })
        .collect();

    let mine: Vec<&Vec<String>> = invocations
        .iter()
        .filter(|argv| argv.first().map(String::as_str) != Some("debug"))
        .collect();
    assert_eq!(
        mine.len(),
        2,
        "expected the build task and the stdlib query; found {invocations:?}; \
         if the extension gained an invocation, cover it here too"
    );

    for argv in mine {
        // `ws` is the extension's variable for the workspace path.
        let argv: Vec<String> = std::iter::once("rk".to_string())
            .chain(argv.iter().map(|a| {
                if a == "ws" {
                    ".".to_string()
                } else {
                    a.clone()
                }
            }))
            .collect();
        Args::try_parse_from(&argv).unwrap_or_else(|e| {
            panic!(
                "the extension runs `{}`, which this CLI rejects:\n{e}",
                argv.join(" ")
            )
        });
    }
}
