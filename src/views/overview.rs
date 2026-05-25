use iced::Element;
use iced::widget::{column, container, row, text};
use parquet::file::metadata::SortingColumn;

use crate::app::Message;
use crate::format::human_bytes;
use crate::parquet_io::FileSummary;

pub fn view(file: &FileSummary) -> Element<'_, Message> {
    let meta = file.metadata.file_metadata();

    let (compressed, uncompressed) = file.metadata.row_groups().iter().fold(
        (0_i64, 0_i64),
        |(c, u), rg| {
            let chunk_c: i64 = rg.columns().iter().map(|cc| cc.compressed_size()).sum();
            let chunk_u: i64 = rg.columns().iter().map(|cc| cc.uncompressed_size()).sum();
            (c + chunk_c, u + chunk_u)
        },
    );

    let mut col = column![
        section("File"),
        kv("Path", file.path.display().to_string()),
        kv("Size on disk", human_bytes(file.file_size_bytes)),
        section("Contents"),
        kv("Total rows", format!("{}", file.total_rows)),
        kv("Row groups", format!("{}", file.metadata.num_row_groups())),
        kv("Columns", format!("{}", file.schema.fields().len())),
        section("Writer"),
        kv("Parquet version", format!("{}", meta.version())),
        kv("Created by", meta.created_by().unwrap_or("(unknown)").to_string()),
        section("Storage"),
        kv("Uncompressed (sum)", human_bytes(uncompressed.max(0) as u64)),
        kv("Compressed (sum)", human_bytes(compressed.max(0) as u64)),
        kv(
            "Compression ratio",
            if compressed > 0 {
                format!("{:.2}x", uncompressed as f64 / compressed as f64)
            } else {
                "—".into()
            },
        ),
    ]
    .spacing(6);

    col = col.push(section("Sort Order"));
    col = col.push(kv("Row groups", sort_order_summary(file)));

    if let Some(kv_pairs) = meta.key_value_metadata() {
        if !kv_pairs.is_empty() {
            col = col.push(section("Key/Value Metadata"));
            for entry in kv_pairs {
                let value = entry.value.clone().unwrap_or_else(|| "(none)".into());
                col = col.push(kv(&entry.key, value));
            }
        }
    }

    col.into()
}

pub fn format_sorting_columns(file: &FileSummary, cols: &[SortingColumn]) -> String {
    if cols.is_empty() {
        return "(none specified)".into();
    }
    cols.iter()
        .map(|sc| {
            let name = file
                .schema
                .fields()
                .get(sc.column_idx as usize)
                .map(|f| f.name().as_str())
                .unwrap_or("?");
            let dir = if sc.descending { "DESC" } else { "ASC" };
            let nulls = if sc.nulls_first { "NULLS FIRST" } else { "NULLS LAST" };
            format!("{name} {dir} {nulls}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn sort_order_summary(file: &FileSummary) -> String {
    let groups = file.metadata.row_groups();
    if groups.is_empty() {
        return "(no row groups)".into();
    }

    let key = |rg: &parquet::file::metadata::RowGroupMetaData| -> Option<Vec<(i32, bool, bool)>> {
        rg.sorting_columns().map(|v| {
            v.iter()
                .map(|sc| (sc.column_idx, sc.descending, sc.nulls_first))
                .collect()
        })
    };

    let first_key = key(&groups[0]);
    let uniform = groups.iter().all(|rg| key(rg) == first_key);

    match (first_key.is_some(), uniform) {
        (false, true) => "(not specified)".into(),
        (true, true) => {
            let cols = groups[0].sorting_columns().unwrap();
            format_sorting_columns(file, cols)
        }
        (_, false) => "(varies by row group — see Row Groups tab)".into(),
    }
}

fn section(title: &str) -> Element<'_, Message> {
    container(text(title.to_string()).size(18))
        .padding([10, 0])
        .into()
}

fn kv<'a>(label: &'a str, value: impl Into<String>) -> Element<'a, Message> {
    let value = value.into();
    row![
        container(text(label.to_string())).width(iced::Length::Fixed(180.0)),
        text(value),
    ]
    .spacing(8)
    .into()
}
