use std::path::Path;

use auto_lsp::{default::db::{FileManager, file::File}, lsp_types::Url};
use db::RootDatabase;

pub fn load_file_into_db(
    db: &mut RootDatabase,
    path: &Path,
) -> Result<File, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let absolute_path = std::fs::canonicalize(path)?;
    let url = Url::from_file_path(&absolute_path)
        .map_err(|_| "Failed to convert path to URL".to_string())?;

    let file = File::from_string()
        .db(db)
        .parsers(
            ast::RK_PARSER
                .get("structured_text")
                .ok_or("Parser not found")?,
        )
        .url(&url)
        .source(content)
        .call()?;

    db.add_file(file)?;;
    Ok(file)
}
