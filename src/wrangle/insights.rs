use std::sync::Arc;

use arrow::array::{Array, Int64Array, UInt64Array};
use arrow::datatypes::DataType;

use crate::wrangle::session::{TABLE_NAME, WrangleSession};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    Numeric,
    Boolean,
    Temporal,
    String,
    Other,
}

pub fn classify(dt: &DataType) -> ColumnKind {
    match dt {
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::Float16
        | DataType::Float32
        | DataType::Float64
        | DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _) => ColumnKind::Numeric,
        DataType::Boolean => ColumnKind::Boolean,
        DataType::Date32
        | DataType::Date64
        | DataType::Time32(_)
        | DataType::Time64(_)
        | DataType::Timestamp(_, _) => ColumnKind::Temporal,
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => ColumnKind::String,
        _ => ColumnKind::Other,
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // name/kind read in later phases (Summary Panel)
pub struct ColumnInsight {
    pub name: String,
    pub kind: ColumnKind,
    pub total: u64,
    pub null_count: u64,
    pub distinct: Option<u64>,
    pub min: Option<String>,
    pub max: Option<String>,
    pub histogram: Option<Histogram>,
    pub top_values: Option<Vec<(String, u64)>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // min/max read for tooltip in later UI iteration
pub struct Histogram {
    pub min: f64,
    pub max: f64,
    pub bin_counts: Vec<u64>,
}

const BUCKETS: usize = 10;
const TOP_K: usize = 3;

pub async fn compute_all(session: Arc<WrangleSession>) -> Result<Vec<ColumnInsight>, String> {
    let schema = session.schema.clone();
    let mut out = Vec::with_capacity(schema.fields().len());
    for field in schema.fields() {
        let kind = classify(field.data_type());
        let insight = compute_one(session.clone(), field.name(), kind).await?;
        out.push(insight);
    }
    Ok(out)
}

async fn compute_one(
    session: Arc<WrangleSession>,
    name: &str,
    kind: ColumnKind,
) -> Result<ColumnInsight, String> {
    let id = ident(name);
    // approx_distinct's HLL impl only accepts a subset of types (Int*/Utf8/Binary).
    // Cast to VARCHAR so the same code path works for Float, Decimal, Date, Timestamp, etc.
    let base_sql = format!(
        "SELECT \
            COUNT(*) AS total, \
            (COUNT(*) - COUNT({id})) AS nulls, \
            approx_distinct(CAST({id} AS VARCHAR)) AS distinct_count, \
            CAST(MIN({id}) AS VARCHAR) AS min_v, \
            CAST(MAX({id}) AS VARCHAR) AS max_v \
         FROM {TABLE_NAME}"
    );
    let df = session
        .ctx
        .sql(&base_sql)
        .await
        .map_err(|e| format!("insights base sql ({name}): {e}"))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| format!("insights base collect ({name}): {e}"))?;
    let BaseStats {
        total,
        nulls,
        distinct,
        min: min_v,
        max: max_v,
    } = parse_base(&batches)?;

    let histogram = if kind == ColumnKind::Numeric {
        match (parse_f64(&min_v), parse_f64(&max_v)) {
            (Some(lo), Some(hi)) if hi > lo => {
                Some(compute_histogram(session.clone(), name, lo, hi).await?)
            }
            (Some(v), Some(_)) => Some(Histogram {
                min: v,
                max: v,
                bin_counts: vec![total.saturating_sub(nulls)],
            }),
            _ => None,
        }
    } else {
        None
    };

    let top_values = if !matches!(kind, ColumnKind::Numeric | ColumnKind::Other) {
        Some(compute_top_values(session.clone(), name).await?)
    } else {
        None
    };

    Ok(ColumnInsight {
        name: name.to_string(),
        kind,
        total,
        null_count: nulls,
        distinct,
        min: min_v,
        max: max_v,
        histogram,
        top_values,
    })
}

struct BaseStats {
    total: u64,
    nulls: u64,
    distinct: Option<u64>,
    min: Option<String>,
    max: Option<String>,
}

fn parse_base(batches: &[arrow::record_batch::RecordBatch]) -> Result<BaseStats, String> {
    let b = batches.first().ok_or("empty insights result")?;
    Ok(BaseStats {
        total: scalar_u64(b.column(0))?,
        nulls: scalar_u64(b.column(1))?,
        distinct: scalar_u64_opt(b.column(2))?,
        min: scalar_string_opt(b.column(3))?,
        max: scalar_string_opt(b.column(4))?,
    })
}

async fn compute_histogram(
    session: Arc<WrangleSession>,
    name: &str,
    lo: f64,
    hi: f64,
) -> Result<Histogram, String> {
    let id = ident(name);
    let span = hi - lo;
    let buckets = BUCKETS as f64;
    let sql = format!(
        "SELECT \
            LEAST(CAST(FLOOR((CAST({id} AS DOUBLE) - {lo}) / ({span} / {buckets})) AS BIGINT), {last_bucket}) AS bucket, \
            COUNT(*) AS cnt \
         FROM {TABLE_NAME} \
         WHERE {id} IS NOT NULL \
         GROUP BY bucket \
         ORDER BY bucket",
        last_bucket = BUCKETS - 1
    );
    let df = session
        .ctx
        .sql(&sql)
        .await
        .map_err(|e| format!("histogram sql ({name}): {e}"))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| format!("histogram collect ({name}): {e}"))?;

    let mut bin_counts = vec![0u64; BUCKETS];
    for batch in batches {
        let bucket = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| "histogram bucket not i64".to_string())?;
        let cnt = batch
            .column(1)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| "histogram cnt not i64".to_string())?;
        for i in 0..batch.num_rows() {
            if bucket.is_null(i) {
                continue;
            }
            let b = bucket.value(i);
            let c = cnt.value(i);
            if (0..BUCKETS as i64).contains(&b) {
                bin_counts[b as usize] += c.max(0) as u64;
            }
        }
    }

    Ok(Histogram {
        min: lo,
        max: hi,
        bin_counts,
    })
}

async fn compute_top_values(
    session: Arc<WrangleSession>,
    name: &str,
) -> Result<Vec<(String, u64)>, String> {
    let id = ident(name);
    let sql = format!(
        "SELECT CAST({id} AS VARCHAR) AS v, COUNT(*) AS cnt \
         FROM {TABLE_NAME} \
         WHERE {id} IS NOT NULL \
         GROUP BY {id} \
         ORDER BY cnt DESC \
         LIMIT {TOP_K}"
    );
    let df = session
        .ctx
        .sql(&sql)
        .await
        .map_err(|e| format!("top values sql ({name}): {e}"))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| format!("top values collect ({name}): {e}"))?;

    let mut out = Vec::new();
    for batch in batches {
        let v_col = batch.column(0);
        let cnt = batch
            .column(1)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| "top cnt not i64".to_string())?;
        for i in 0..batch.num_rows() {
            let value = if v_col.is_null(i) {
                "∅".into()
            } else {
                arrow::util::display::ArrayFormatter::try_new(
                    v_col.as_ref(),
                    &arrow::util::display::FormatOptions::default(),
                )
                .ok()
                .and_then(|f| f.value(i).try_to_string().ok())
                .unwrap_or_default()
            };
            out.push((value, cnt.value(i).max(0) as u64));
        }
    }
    Ok(out)
}

fn ident(name: &str) -> String {
    let escaped = name.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

fn parse_f64(s: &Option<String>) -> Option<f64> {
    s.as_ref().and_then(|v| v.parse::<f64>().ok())
}

fn scalar_u64(arr: &Arc<dyn Array>) -> Result<u64, String> {
    if let Some(a) = arr.as_any().downcast_ref::<Int64Array>() {
        if a.is_empty() {
            return Ok(0);
        }
        return Ok(a.value(0).max(0) as u64);
    }
    if let Some(a) = arr.as_any().downcast_ref::<UInt64Array>() {
        if a.is_empty() {
            return Ok(0);
        }
        return Ok(a.value(0));
    }
    Err(format!("unexpected aggregate type: {:?}", arr.data_type()))
}

fn scalar_u64_opt(arr: &Arc<dyn Array>) -> Result<Option<u64>, String> {
    if let Some(a) = arr.as_any().downcast_ref::<Int64Array>() {
        if a.is_empty() || a.is_null(0) {
            return Ok(None);
        }
        return Ok(Some(a.value(0).max(0) as u64));
    }
    if let Some(a) = arr.as_any().downcast_ref::<UInt64Array>() {
        if a.is_empty() || a.is_null(0) {
            return Ok(None);
        }
        return Ok(Some(a.value(0)));
    }
    Err(format!("unexpected scalar type: {:?}", arr.data_type()))
}

fn scalar_string_opt(arr: &Arc<dyn Array>) -> Result<Option<String>, String> {
    if arr.is_empty() || arr.is_null(0) {
        return Ok(None);
    }
    let fmt = arrow::util::display::ArrayFormatter::try_new(
        arr.as_ref(),
        &arrow::util::display::FormatOptions::default(),
    )
    .map_err(|e| format!("string formatter: {e}"))?;
    Ok(fmt.value(0).try_to_string().ok())
}
