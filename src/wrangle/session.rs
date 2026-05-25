use std::path::PathBuf;
use std::sync::Arc;

use arrow::datatypes::SchemaRef;
use arrow::record_batch::RecordBatch;
use datafusion::execution::config::SessionConfig;
use datafusion::prelude::{ParquetReadOptions, SessionContext};

pub const TABLE_NAME: &str = "t";

pub struct WrangleSession {
    pub ctx: SessionContext,
    pub path: PathBuf,
    pub schema: SchemaRef,
}

impl std::fmt::Debug for WrangleSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WrangleSession")
            .field("path", &self.path)
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl WrangleSession {
    pub async fn open(path: PathBuf) -> Result<Arc<Self>, String> {
        let path_str = path
            .to_str()
            .ok_or_else(|| "path is not valid UTF-8".to_string())?
            .to_string();
        tracing::info!(path = %path.display(), "opening wrangle session");
        // Pin to a single partition so a `SELECT *` over a sorted parquet file
        // returns rows in storage order. With the default (CPU-count) partitioning,
        // row groups get scanned in parallel and interleaved on output.
        let config = SessionConfig::new()
            .with_target_partitions(1)
            .with_repartition_file_scans(false);
        let ctx = SessionContext::new_with_config(config);
        ctx.register_parquet(TABLE_NAME, &path_str, ParquetReadOptions::default())
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "register parquet failed");
                format!("register parquet: {e}")
            })?;
        let df = ctx
            .table(TABLE_NAME)
            .await
            .map_err(|e| format!("table: {e}"))?;
        let schema: SchemaRef = Arc::new(df.schema().as_arrow().clone());
        Ok(Arc::new(Self { ctx, path, schema }))
    }

    pub async fn count_rows(self: Arc<Self>) -> Result<i64, String> {
        let df = self
            .ctx
            .table(TABLE_NAME)
            .await
            .map_err(|e| format!("table: {e}"))?;
        let n = df.count().await.map_err(|e| format!("count: {e}"))?;
        Ok(n as i64)
    }

    pub async fn collect_sql_page(
        self: Arc<Self>,
        base_sql: String,
        offset: usize,
        limit: usize,
    ) -> Result<(RecordBatch, SchemaRef), String> {
        let wrapped = format!("SELECT * FROM ({base_sql}) LIMIT {limit} OFFSET {offset}");
        tracing::debug!(offset, limit, sql = %wrapped, "collect_sql_page");
        let df = self.ctx.sql(&wrapped).await.map_err(|e| {
            tracing::warn!(error = %e, sql = %wrapped, "pipeline sql failed");
            format!("pipeline sql: {e}")
        })?;
        let schema: SchemaRef = Arc::new(df.schema().as_arrow().clone());
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("pipeline collect: {e}"))?;
        let batch = concat_or_empty(schema.clone(), batches)?;
        tracing::debug!(rows = batch.num_rows(), "collected page");
        Ok((batch, schema))
    }

    pub async fn count_sql(self: Arc<Self>, base_sql: String) -> Result<i64, String> {
        let wrapped = format!("SELECT COUNT(*) FROM ({base_sql})");
        let df = self
            .ctx
            .sql(&wrapped)
            .await
            .map_err(|e| format!("count sql: {e}"))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| format!("count collect: {e}"))?;
        let b = batches.first().ok_or("empty count result")?;
        let col = b.column(0);
        if let Some(a) = col.as_any().downcast_ref::<arrow::array::Int64Array>() {
            if a.is_empty() {
                return Ok(0);
            }
            return Ok(a.value(0));
        }
        Err(format!("unexpected count type: {:?}", col.data_type()))
    }
}

fn concat_or_empty(schema: SchemaRef, batches: Vec<RecordBatch>) -> Result<RecordBatch, String> {
    if batches.is_empty() {
        return Ok(RecordBatch::new_empty(schema));
    }
    if batches.len() == 1 {
        return Ok(batches.into_iter().next().unwrap());
    }
    arrow::compute::concat_batches(&schema, &batches).map_err(|e| format!("concat: {e}"))
}
