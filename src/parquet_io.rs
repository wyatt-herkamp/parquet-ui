use ahash::{HashSet, HashSetExt};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use arrow::array::{Array, RecordBatchReader};
use arrow::datatypes::SchemaRef;
use arrow::record_batch::RecordBatch;
use arrow::row::{RowConverter, SortField};
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

#[derive(Debug, Clone)]
pub struct ColumnStats {
    pub null_count: Option<i64>,
    pub total_count: i64,
    pub distinct_count: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct FileStats {
    pub columns: Vec<ColumnStats>,
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

pub async fn load_page(path: PathBuf, offset: usize, limit: usize) -> Result<RecordBatch, String> {
    tokio::task::spawn_blocking(move || load_page_blocking(path, offset, limit))
        .await
        .map_err(|e| format!("join error: {e}"))?
}

fn load_page_blocking(path: PathBuf, offset: usize, limit: usize) -> Result<RecordBatch, String> {
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
    arrow::compute::concat_batches(&schema, &batches).map_err(|e| format!("concat failed: {e}"))
}

/// Sentinel error returned when a distinct-count task is cancelled; UI drops it silently.
pub const CANCELLED: &str = "cancelled";

pub fn quick_stats(summary: &FileSummary) -> FileStats {
    let num_cols = summary.schema.fields().len();
    let mut nulls = vec![0i64; num_cols];
    let mut known = vec![true; num_cols];

    for rg in summary.metadata.row_groups() {
        for (c, cc) in rg.columns().iter().enumerate().take(num_cols) {
            match cc.statistics().and_then(|s| s.null_count_opt()) {
                Some(n) => nulls[c] += n as i64,
                None => known[c] = false,
            }
        }
    }

    let columns = (0..num_cols)
        .map(|i| ColumnStats {
            null_count: if known[i] { Some(nulls[i]) } else { None },
            total_count: summary.total_rows,
            distinct_count: None,
        })
        .collect();

    FileStats { columns }
}

// Detached std::thread (not tokio spawn_blocking) so window close doesn't wait on it.
pub async fn compute_distinct(
    path: PathBuf,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<usize>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result = compute_distinct_blocking(path, cancel);
        let _ = tx.send(result);
    });
    match rx.await {
        Ok(r) => r,
        Err(_) => Err(CANCELLED.into()),
    }
}

fn compute_distinct_blocking(path: PathBuf, cancel: Arc<AtomicBool>) -> Result<Vec<usize>, String> {
    let file = File::open(&path).map_err(|e| format!("open failed: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("parquet open failed: {e}"))?;

    let schema = builder.schema().clone();
    let num_cols = schema.fields().len();

    // Row format gives same-value -> same-bytes for any data type, no per-value formatting.
    let converters: Vec<RowConverter> = schema
        .fields()
        .iter()
        .map(|f| RowConverter::new(vec![SortField::new(f.data_type().clone())]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("row converter: {e}"))?;

    let reader = builder
        .with_batch_size(8192)
        .build()
        .map_err(|e| format!("reader build failed: {e}"))?;

    let mut distincts: Vec<HashSet<Box<[u8]>>> = (0..num_cols).map(|_| HashSet::new()).collect();

    for batch in reader {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        let batch = batch.map_err(|e| format!("batch read failed: {e}"))?;
        for c in 0..num_cols {
            let arr = batch.column(c);
            let rows = converters[c]
                .convert_columns(&[arr.clone()])
                .map_err(|e| format!("row convert: {e}"))?;
            let set = &mut distincts[c];
            for i in 0..arr.len() {
                if arr.is_null(i) {
                    continue;
                }
                let row = rows.row(i);
                let row_bytes: &[u8] = row.as_ref();
                if !set.contains(row_bytes) {
                    set.insert(row_bytes.into());
                }
            }
        }
    }

    Ok(distincts.into_iter().map(|s| s.len()).collect())
}
