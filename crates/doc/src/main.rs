mod examples;
mod render;

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

    // Group by category, preserving order within each category
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

    // Build all entry HTML fragments grouped by category + JSON API
    let mut body = String::new();
    let mut json_entries: Vec<String> = Vec::new();
    let mut success_count = 0;
    let mut fail_count = 0;
    let mut current_category = "";

    for ex in &examples {
        // Insert category heading when category changes
        if ex.category != current_category {
            current_category = ex.category;
            let slug = current_category.to_lowercase().replace(' ', "-");
            body.push_str(&format!(
                "<h1 class=\"category-heading\" id=\"cat-{slug}\">{current_category}</h1>\n"
            ));
        }

        eprint!("  Generating {}...", ex.code);

        let mut db = RootDatabase::default();
        let (ansi_output, diag_spans) =
            render::compile_and_render(&mut db, ex.sources, ex.lint_rule);

        if ansi_output.is_empty() {
            eprintln!(" WARNING: no diagnostics produced!");
            fail_count += 1;

            body.push_str(&render::render_entry_html(
                ex.code,
                ex.title,
                ex.description,
                ex.sources,
                "<span class=\"no-output\">No compiler output - example may need updating.</span>",
                &[],
            ));
            continue;
        }

        let report_html = render::ansi_to_html_fragment(&ansi_output);
        body.push_str(&render::render_entry_html(
            ex.code,
            ex.title,
            ex.description,
            ex.sources,
            &report_html,
            &diag_spans,
        ));

        // Collect JSON entry for API
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

    // Build sidebar
    let sidebar = render::render_sidebar_html(&ordered_categories);

    // Assemble full page
    let page = render::render_page(&sidebar, &body, TM_GRAMMAR, success_count, fail_count);

    let output_file = out_dir.join("reference.html");
    fs::write(&output_file, &page).expect("failed to write reference.html");

    // Write JSON API for agents
    let json_file = out_dir.join("diagnostics.json");
    let json_content = format!("[\n{}\n]\n", json_entries.join(",\n"));
    fs::write(&json_file, &json_content).expect("failed to write diagnostics.json");

    eprintln!("\nDone! {success_count} diagnostics documented, {fail_count} warnings.");
    eprintln!("Output: {}", output_file.display());
    eprintln!("API:    {}", json_file.display());
}
