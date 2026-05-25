use iced::widget::container::Style as ContainerStyle;
use iced::widget::text::Wrapping;
use iced::widget::{column, container, mouse_area, row, text};
use iced::{Background, Border, Element, Length, Theme};
use parquet::file::metadata::SortingColumn;

use crate::app::Message;
use crate::format::human_bytes;
use crate::parquet_io::FileSummary;

pub fn view(file: &FileSummary) -> Element<'_, Message> {
    let meta = file.metadata.file_metadata();

    let (compressed, uncompressed) =
        file.metadata
            .row_groups()
            .iter()
            .fold((0_i64, 0_i64), |(c, u), rg| {
                let chunk_c: i64 = rg.columns().iter().map(|cc| cc.compressed_size()).sum();
                let chunk_u: i64 = rg.columns().iter().map(|cc| cc.uncompressed_size()).sum();
                (c + chunk_c, u + chunk_u)
            });

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
        kv(
            "Created by",
            meta.created_by().unwrap_or("(unknown)").to_string()
        ),
        section("Storage"),
        kv(
            "Uncompressed (sum)",
            human_bytes(uncompressed.max(0) as u64)
        ),
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

    if let Some(kv_pairs) = meta.key_value_metadata()
        && !kv_pairs.is_empty()
    {
        col = col.push(section("Key/Value Metadata"));
        for entry in kv_pairs {
            let value = entry.value.clone().unwrap_or_else(|| "(none)".into());
            col = col.push(kv(&entry.key, value));
        }
    }

    col = col.push(section("Schema"));
    col = col.push(schema_table(file));

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
            let nulls = if sc.nulls_first {
                "NULLS FIRST"
            } else {
                "NULLS LAST"
            };
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

const SCHEMA_HEADERS: [&str; 7] = [
    "Column",
    "Physical",
    "Logical",
    "Arrow type",
    "Nullable",
    "Max def",
    "Max rep",
];

const SCHEMA_COL_WIDTHS: [f32; 7] = [240.0, 110.0, 220.0, 260.0, 80.0, 80.0, 80.0];

fn schema_table(file: &FileSummary) -> Element<'_, Message> {
    let schema_descr = file.metadata.file_metadata().schema_descr();
    let num_cols = schema_descr.num_columns();

    let mut rows = column![schema_header_row()].spacing(0);
    for i in 0..num_cols {
        let col_desc = schema_descr.column(i);
        let arrow_field = file.schema.fields().get(i);

        let name = col_desc.path().string();
        let physical = format!("{:?}", col_desc.physical_type());
        let logical = col_desc
            .logical_type_ref()
            .map(|l| format!("{l:?}"))
            .unwrap_or_else(|| format!("{:?}", col_desc.converted_type()));
        let arrow_type = arrow_field
            .map(|f| format!("{:?}", f.data_type()))
            .unwrap_or_else(|| "—".into());
        let nullable = arrow_field
            .map(|f| if f.is_nullable() { "yes" } else { "no" }.to_string())
            .unwrap_or_else(|| "—".into());
        let max_def = col_desc.max_def_level().to_string();
        let max_rep = col_desc.max_rep_level().to_string();

        rows = rows.push(schema_data_row(
            [
                name, physical, logical, arrow_type, nullable, max_def, max_rep,
            ],
            i % 2 == 1,
        ));
    }

    rows.into()
}

fn schema_header_row() -> Element<'static, Message> {
    let mut r = row![].spacing(0);
    for (i, h) in SCHEMA_HEADERS.iter().enumerate() {
        r = r.push(schema_header_cell(h, SCHEMA_COL_WIDTHS[i]));
    }
    container(r).style(schema_header_row_style).into()
}

fn schema_data_row<'a>(values: [String; 7], zebra: bool) -> Element<'a, Message> {
    let mut r = row![].spacing(0);
    for (i, v) in values.into_iter().enumerate() {
        r = r.push(schema_body_cell(v, SCHEMA_COL_WIDTHS[i]));
    }
    container(r)
        .style(move |theme: &Theme| schema_body_row_style(theme, zebra))
        .into()
}

fn schema_header_cell<'a>(s: &str, width: f32) -> Element<'a, Message> {
    container(text(s.to_string()).size(13).wrapping(Wrapping::None))
        .width(Length::Fixed(width))
        .padding([6, 10])
        .clip(true)
        .into()
}

fn schema_body_cell<'a>(value: String, width: f32) -> Element<'a, Message> {
    let label = text(value.clone()).size(13).wrapping(Wrapping::None);
    let inner = container(label)
        .width(Length::Fixed(width))
        .padding([4, 10])
        .clip(true);
    mouse_area(inner).on_press(Message::CopyCell(value)).into()
}

fn schema_header_row_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.strong.color)),
        text_color: Some(p.background.strong.text),
        border: Border::default(),
        ..ContainerStyle::default()
    }
}

fn schema_body_row_style(theme: &Theme, zebra: bool) -> ContainerStyle {
    let p = theme.extended_palette();
    let bg = if zebra {
        p.background.weak.color
    } else {
        p.background.base.color
    };
    ContainerStyle {
        background: Some(Background::Color(bg)),
        text_color: Some(p.background.base.text),
        border: Border::default(),
        ..ContainerStyle::default()
    }
}
