mod examples;
mod render;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use db::RootDatabase;

const TM_GRAMMAR: &str = include_str!("../../../vscode/syntaxes/st.tmLanguage.json");

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
        .map(|cat| (*cat, categories.get(cat).cloned().unwrap_or_default()))
        .collect();

    // Build all entry HTML fragments grouped by category
    let mut body = String::new();
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
        let run_linter = ex.code.starts_with('W');
        let ansi_output = render::compile_and_render(&mut db, ex.sources, run_linter);

        if ansi_output.is_empty() {
            eprintln!(" WARNING: no diagnostics produced!");
            fail_count += 1;

            body.push_str(&render::render_entry_html(
                ex.code,
                ex.title,
                ex.description,
                ex.sources,
                "<span class=\"warning\">No compiler output — example may need updating.</span>",
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
        ));

        success_count += 1;
        eprintln!(" ok");
    }

    // Build sidebar
    let sidebar = render::render_sidebar_html(&ordered_categories);

    // Assemble full page
    let page = render::render_page(&sidebar, &body, TM_GRAMMAR, success_count, fail_count);

    let output_file = out_dir.join("index.html");
    fs::write(&output_file, &page).expect("failed to write index.html");

    eprintln!(
        "\nDone! {success_count} diagnostics documented, {fail_count} warnings."
    );
    eprintln!("Output: {}", output_file.display());
}
