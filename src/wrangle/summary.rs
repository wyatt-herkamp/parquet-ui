use std::sync::Arc;

use arrow::array::{Array, Float64Array, Int64Array, UInt64Array};

use crate::wrangle::insights::{ColumnKind, classify};
use crate::wrangle::session::{TABLE_NAME, WrangleSession};

const TOP_K: usize = 10;

#[derive(Debug, Clone)]
#[allow(dead_code)] // name and kind are surfaced contextually but not read directly yet
pub struct ColumnSummary {
    pub name: String,
    pub kind: ColumnKind,
    pub total: u64,
    pub null_count: u64,
    pub distinct: Option<u64>,
    pub min: Option<String>,
    pub max: Option<String>,
    pub mean: Option<f64>,
    pub std: Option<f64>,
    pub q25: Option<f64>,
    pub q50: Option<f64>,
    pub q75: Option<f64>,
    pub top_values: Vec<(String, u64)>,
}

pub async fn compute(
    session: Arc<WrangleSession>,
    column_index: usize,
) -> Result<ColumnSummary, String> {
    let schema = session.schema.clone();
    let field = schema
        .fields()
        .get(column_index)
        .ok_or_else(|| format!("column index {column_index} out of range"))?;
    let name = field.name().to_string();
    let kind = classify(field.data_type());

    let id = ident(&name);
    let numeric = matches!(kind, ColumnKind::Numeric);

    let mut base_select = format!(
        "SELECT \
            COUNT(*) AS total, \
            (COUNT(*) - COUNT({id})) AS nulls, \
            approx_distinct(CAST({id} AS VARCHAR)) AS distinct_count, \
            CAST(MIN({id}) AS VARCHAR) AS min_v, \
            CAST(MAX({id}) AS VARCHAR) AS max_v"
    );
    if numeric {
        base_select.push_str(&format!(
            ", AVG(CAST({id} AS DOUBLE)) AS mean_v, \
              STDDEV(CAST({id} AS DOUBLE)) AS std_v, \
              approx_percentile_cont(CAST({id} AS DOUBLE), 0.25) AS q25_v, \
              approx_percentile_cont(CAST({id} AS DOUBLE), 0.5) AS q50_v, \
              approx_percentile_cont(CAST({id} AS DOUBLE), 0.75) AS q75_v"
        ));
    }
    base_select.push_str(&format!(" FROM {TABLE_NAME}"));

    let df = session
        .ctx
        .sql(&base_select)
        .await
        .map_err(|e| format!("summary sql: {e}"))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| format!("summary collect: {e}"))?;
    let b = batches.first().ok_or("empty summary result")?;

    let total = scalar_u64(b.column(0))?;
    let nulls = scalar_u64(b.column(1))?;
    let distinct = scalar_u64_opt(b.column(2))?;
    let min_v = scalar_string_opt(b.column(3))?;
    let max_v = scalar_string_opt(b.column(4))?;
    let (mean, std, q25, q50, q75) = if numeric {
        (
            scalar_f64_opt(b.column(5))?,
            scalar_f64_opt(b.column(6))?,
            scalar_f64_opt(b.column(7))?,
            scalar_f64_opt(b.column(8))?,
            scalar_f64_opt(b.column(9))?,
        )
    } else {
        (None, None, None, None, None)
    };

    let top_values = compute_top(session.clone(), &name)
        .await
        .unwrap_or_default();

    Ok(ColumnSummary {
        name,
        kind,
        total,
        null_count: nulls,
        distinct,
        min: min_v,
        max: max_v,
        mean,
        std,
        q25,
        q50,
        q75,
        top_values,
    })
}

async fn compute_top(
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
        .map_err(|e| format!("top sql: {e}"))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| format!("top collect: {e}"))?;

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

fn scalar_f64_opt(arr: &Arc<dyn Array>) -> Result<Option<f64>, String> {
    if let Some(a) = arr.as_any().downcast_ref::<Float64Array>() {
        if a.is_empty() || a.is_null(0) {
            return Ok(None);
        }
        return Ok(Some(a.value(0)));
    }
    Err(format!("expected float64: {:?}", arr.data_type()))
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
