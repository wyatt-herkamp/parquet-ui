use iced::widget::container::Style as ContainerStyle;
use iced::widget::text::Wrapping;
use iced::widget::{column, container, mouse_area, row, text};
use iced::{Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::parquet_io::FileSummary;

pub fn view(file: &FileSummary) -> Element<'_, Message> {
    let schema_descr = file.metadata.file_metadata().schema_descr();
    let num_cols = schema_descr.num_columns();

    let mut rows = column![header_row()].spacing(0);
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

        rows = rows.push(data_row(
            [name, physical, logical, arrow_type, nullable, max_def, max_rep],
            i % 2 == 1,
        ));
    }

    rows.into()
}

const HEADERS: [&str; 7] = [
    "Column",
    "Physical",
    "Logical",
    "Arrow type",
    "Nullable",
    "Max def",
    "Max rep",
];

const COL_WIDTHS: [f32; 7] = [240.0, 110.0, 220.0, 260.0, 80.0, 80.0, 80.0];

fn header_row() -> Element<'static, Message> {
    let mut r = row![].spacing(0);
    for (i, h) in HEADERS.iter().enumerate() {
        r = r.push(header_cell(h, COL_WIDTHS[i]));
    }
    container(r).style(header_row_style).into()
}

fn data_row<'a>(values: [String; 7], zebra: bool) -> Element<'a, Message> {
    let mut r = row![].spacing(0);
    for (i, v) in values.into_iter().enumerate() {
        r = r.push(body_cell(v, COL_WIDTHS[i]));
    }
    container(r)
        .style(move |theme: &Theme| body_row_style(theme, zebra))
        .into()
}

fn header_cell<'a>(s: &str, width: f32) -> Element<'a, Message> {
    container(text(s.to_string()).size(13).wrapping(Wrapping::None))
        .width(Length::Fixed(width))
        .padding([6, 10])
        .clip(true)
        .into()
}

fn body_cell<'a>(value: String, width: f32) -> Element<'a, Message> {
    let label = text(value.clone()).size(13).wrapping(Wrapping::None);
    let inner = container(label)
        .width(Length::Fixed(width))
        .padding([4, 10])
        .clip(true);
    mouse_area(inner).on_press(Message::CopyCell(value)).into()
}

fn header_row_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.strong.color)),
        text_color: Some(p.background.strong.text),
        border: Border::default(),
        ..ContainerStyle::default()
    }
}

fn body_row_style(theme: &Theme, zebra: bool) -> ContainerStyle {
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
