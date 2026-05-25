use std::path::PathBuf;
use std::sync::Arc;

use datafusion::dataframe::DataFrameWriteOptions;

use crate::wrangle::session::WrangleSession;

#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Parquet,
    Csv,
    Json,
}

impl ExportFormat {
    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("parquet") => Some(ExportFormat::Parquet),
            Some(e) if e.eq_ignore_ascii_case("csv") => Some(ExportFormat::Csv),
            Some(e)
                if e.eq_ignore_ascii_case("json")
                    || e.eq_ignore_ascii_case("jsonl")
                    || e.eq_ignore_ascii_case("ndjson") =>
            {
                Some(ExportFormat::Json)
            }
            _ => None,
        }
    }
}

pub async fn export(
    session: Arc<WrangleSession>,
    sql: String,
    path: PathBuf,
) -> Result<PathBuf, String> {
    let format = ExportFormat::from_path(&path)
        .ok_or_else(|| "unrecognized extension (use .parquet, .csv, or .json)".to_string())?;
    let path_str = path
        .to_str()
        .ok_or_else(|| "destination path is not valid UTF-8".to_string())?
        .to_string();

    tracing::info!(dest = %path.display(), ?format, "exporting wrangle result");

    let df = session
        .ctx
        .sql(&sql)
        .await
        .map_err(|e| format!("build query: {e}"))?;

    let options = DataFrameWriteOptions::new().with_single_file_output(true);

    match format {
        ExportFormat::Parquet => df
            .write_parquet(&path_str, options, None)
            .await
            .map_err(|e| format!("write parquet: {e}"))?,
        ExportFormat::Csv => df
            .write_csv(&path_str, options, None)
            .await
            .map_err(|e| format!("write csv: {e}"))?,
        ExportFormat::Json => df
            .write_json(&path_str, options, None)
            .await
            .map_err(|e| format!("write json: {e}"))?,
    };

    tracing::info!(dest = %path.display(), "export complete");
    Ok(path)
}
