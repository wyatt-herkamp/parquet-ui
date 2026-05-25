use arrow::array::Array;
use arrow::record_batch::RecordBatch;
use arrow::util::display::{ArrayFormatter, FormatOptions};

pub fn default_options() -> FormatOptions<'static> {
    FormatOptions::default()
        .with_display_error(true)
        .with_null("∅")
}

pub fn cell(array: &dyn Array, row: usize, opts: &FormatOptions) -> String {
    match ArrayFormatter::try_new(array, opts) {
        Ok(fmt) => fmt
            .value(row)
            .try_to_string()
            .unwrap_or_else(|e| format!("<err: {e}>")),
        Err(e) => format!("<unsupported: {e}>"),
    }
}

pub fn row_strings(batch: &RecordBatch, row: usize, opts: &FormatOptions) -> Vec<String> {
    (0..batch.num_columns())
        .map(|c| cell(batch.column(c).as_ref(), row, opts))
        .collect()
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", value, UNITS[unit])
    }
}
