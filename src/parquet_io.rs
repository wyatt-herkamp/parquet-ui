use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::RecordBatchReader;
use arrow::datatypes::SchemaRef;
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::metadata::ParquetMetaData;

#[derive(Debug, Clone)]
pub struct FileSummary {
    pub path: PathBuf,
    pub metadata: Arc<ParquetMetaData>,
    pub schema: SchemaRef,
    pub total_rows: i64,
    pub file_size_bytes: u64,
}

pub async fn load_metadata(path: PathBuf) -> Result<FileSummary, String> {
    tokio::task::spawn_blocking(move || load_metadata_blocking(path))
        .await
        .map_err(|e| format!("join error: {e}"))?
}

fn load_metadata_blocking(path: PathBuf) -> Result<FileSummary, String> {
    let file_size_bytes = std::fs::metadata(&path)
        .map_err(|e| format!("stat failed: {e}"))?
        .len();

    let file = File::open(&path).map_err(|e| format!("open failed: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("not a valid parquet file: {e}"))?;

    let schema = builder.schema().clone();
    let metadata = builder.metadata().clone();
    let total_rows = metadata.file_metadata().num_rows();

    Ok(FileSummary {
        path,
        metadata,
        schema,
        total_rows,
        file_size_bytes,
    })
}

pub async fn load_page(
    path: PathBuf,
    offset: usize,
    limit: usize,
) -> Result<RecordBatch, String> {
    tokio::task::spawn_blocking(move || load_page_blocking(path, offset, limit))
        .await
        .map_err(|e| format!("join error: {e}"))?
}

fn load_page_blocking(
    path: PathBuf,
    offset: usize,
    limit: usize,
) -> Result<RecordBatch, String> {
    let file = File::open(&path).map_err(|e| format!("open failed: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("parquet open failed: {e}"))?;

    let reader = builder
        .with_offset(offset)
        .with_limit(limit)
        .with_batch_size(limit.max(1))
        .build()
        .map_err(|e| format!("reader build failed: {e}"))?;

    let schema = reader.schema();
    let mut batches: Vec<RecordBatch> = Vec::new();
    for batch in reader {
        batches.push(batch.map_err(|e| format!("batch read failed: {e}"))?);
    }

    if batches.is_empty() {
        return Ok(RecordBatch::new_empty(schema));
    }
    if batches.len() == 1 {
        return Ok(batches.into_iter().next().unwrap());
    }
    arrow::compute::concat_batches(&schema, &batches)
        .map_err(|e| format!("concat failed: {e}"))
}
