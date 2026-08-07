mod examples;
mod render;
mod verify;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use db::RootDatabase;

const TM_GRAMMAR: &str = include_str!("../../../vscode/syntaxes/st.tmLanguage.json");

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn main() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir = std::env::args()
        .nth(1)
        .map(|s| Path::new(&s).to_path_buf())
        .unwrap_or_else(|| crate_dir.join("out"));

    fs::create_dir_all(&out_dir).expect("failed to create output directory");

    let examples = examples::all_examples();

    // Index examples by code for ordered iteration
    let example_by_code: BTreeMap<&str, &examples::ErrorExample> =
        examples.iter().map(|ex| (ex.code, ex)).collect();

    // Group by category, preserving first-seen category order
    let mut categories: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    let mut category_order: Vec<&str> = Vec::new();

    for ex in &examples {
        if !categories.contains_key(ex.category) {
            category_order.push(ex.category);
        }
        categories
            .entry(ex.category)
            .or_default()
            .push((ex.code, ex.title));
    }

    let ordered_categories: Vec<(&str, Vec<(&str, &str)>)> = category_order
        .iter()
        .map(|cat| {
            let mut entries = categories.get(cat).cloned().unwrap_or_default();
            entries.sort_by_key(|(code, _)| *code);
            (*cat, entries)
        })
        .collect();

    // Build body in the same order as the sidebar (sorted by code within each category)
    let mut body = String::new();
    let mut json_entries: Vec<String> = Vec::new();
    let mut success_count = 0;
    // What each example actually produced, for the alignment check below.
    let mut produced: Vec<(&str, std::collections::BTreeSet<String>)> = Vec::new();

    for (category, entries) in &ordered_categories {
        let slug = category.to_lowercase().replace(' ', "-");
        body.push_str(&format!(
            "<h1 class=\"category-heading\" id=\"cat-{slug}\">{category}</h1>\n"
        ));

        for (code, _) in entries {
            let ex = example_by_code
                .get(code)
                .expect("example must exist for sidebar entry");

            eprint!("  Generating {}...", ex.code);

            let mut db = RootDatabase::default();
            let (ansi_output, diag_spans) =
                render::compile_and_render(&mut db, ex.sources, ex.lint_rule);

            produced.push((ex.code, verify::codes_in_output(&ansi_output)));

            let report_html = render::ansi_to_html_fragment(&ansi_output);
            body.push_str(&render::render_entry_html(
                ex.code,
                ex.title,
                ex.description,
                ex.sources,
                &report_html,
                &diag_spans,
            ));

            let sources_json: Vec<String> = ex
                .sources
                .iter()
                .map(|s| {
                    let trimmed = s.trim_matches('\n');
                    format!("\"{}\"", json_escape(trimmed))
                })
                .collect();
            json_entries.push(format!(
                r#"  {{"code":"{}","category":"{}","title":"{}","description":"{}","sources":[{}]}}"#,
                json_escape(ex.code),
                json_escape(ex.category),
                json_escape(ex.title),
                json_escape(ex.description),
                sources_json.join(","),
            ));

            success_count += 1;
            eprintln!(" ok");
        }
    }

    // The reference must agree with the compiler before it replaces the
    // committed one — `rk explain` embeds the JSON, so publishing a wrong
    // entry hands a user a wrong answer. Checked here rather than in a unit
    // test so that generating the docs at all is what enforces it.
    let problems = verify::problems(&examples, &produced);
    if !problems.is_empty() {
        eprintln!(
            "\nThe diagnostics reference disagrees with the compiler in {} place(s); \
             nothing was written.\n",
            problems.len()
        );
        for problem in &problems {
            eprintln!("  {problem}");
        }
        eprintln!(
            "\nEach example must produce the diagnostic it documents, and every code the \
             compiler\ndefines must have one. Fix the example (or the code), then re-run."
        );
        std::process::exit(1);
    }

    // Build sidebar
    let sidebar = render::render_sidebar_html(&ordered_categories);

    // Assemble full page
    let page = render::render_page(&sidebar, &body, TM_GRAMMAR, success_count, 0);

    let output_file = out_dir.join("reference.html");
    fs::write(&output_file, &page).expect("failed to write reference.html");

    // Write JSON API for agents
    let json_file = out_dir.join("diagnostics.json");
    let json_content = format!("[\n{}\n]\n", json_entries.join(",\n"));
    fs::write(&json_file, &json_content).expect("failed to write diagnostics.json");

    eprintln!("\nDone! {success_count} diagnostics documented, all aligned with the compiler.");
    eprintln!("Output: {}", output_file.display());
    eprintln!("API:    {}", json_file.display());
}
