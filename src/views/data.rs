use arrow::record_batch::RecordBatch;
use iced::widget::container::Style as ContainerStyle;
use iced::widget::text::Wrapping;
use iced::widget::{Space, button, column, container, mouse_area, row, text, text_input, tooltip};
use iced::{Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::format::{default_options, row_strings};
use crate::parquet_io::FileSummary;

const CELL_WIDTH: f32 = 180.0;
const ROW_NUMBER_WIDTH: f32 = 70.0;
const ROW_HEIGHT: f32 = 26.0;
const HEADER_HEIGHT: f32 = 30.0;

pub fn view<'a>(
    file: &'a FileSummary,
    batch: Option<&'a RecordBatch>,
    page: usize,
    page_size: usize,
    page_size_input: &'a str,
    loading: bool,
    total_pages: usize,
) -> Element<'a, Message> {
    let footer = view_footer(page, page_size_input, loading, total_pages);

    let body: Element<'a, Message> = match batch {
        Some(b) if b.num_rows() > 0 => view_grid(b, page, page_size),
        Some(_) => container(text("(no rows on this page)")).padding(12).into(),
        None => {
            let msg = if loading {
                "Loading…"
            } else {
                "No data loaded yet."
            };
            container(text(msg)).padding(12).into()
        }
    };

    let summary = text(format!(
        "{} rows · {} columns · page size {} · click any cell to copy",
        file.total_rows,
        file.schema.fields().len(),
        page_size
    ))
    .size(13);

    column![summary, footer, body].spacing(8).into()
}

fn view_grid<'a>(batch: &'a RecordBatch, page: usize, page_size: usize) -> Element<'a, Message> {
    let opts = default_options();
    let schema = batch.schema();

    let mut header = row![header_cell("#", ROW_NUMBER_WIDTH)].spacing(0);
    for f in schema.fields() {
        header = header.push(header_cell_with_tooltip(
            f.name(),
            CELL_WIDTH,
            &format!("{}", f.data_type()),
        ));
    }
    let header: Element<'a, Message> = container(header)
        .height(Length::Fixed(HEADER_HEIGHT))
        .style(header_row_style)
        .into();

    let row_offset = page * page_size;
    let mut rows_col = column![header].spacing(0);
    for r in 0..batch.num_rows() {
        let values = row_strings(batch, r, &opts);
        let zebra = r % 2 == 1;

        let mut row_widgets = row![row_number_cell(row_offset + r + 1, zebra)].spacing(0);
        for v in values {
            row_widgets = row_widgets.push(body_cell(v, CELL_WIDTH, zebra));
        }
        let styled_row = container(row_widgets)
            .height(Length::Fixed(ROW_HEIGHT))
            .style(move |theme: &Theme| body_row_style(theme, zebra));
        rows_col = rows_col.push(styled_row);
    }

    rows_col.into()
}

fn header_cell<'a>(label: &str, width: f32) -> Element<'a, Message> {
    container(text(label.to_string()).size(13).wrapping(Wrapping::None))
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding([6, 10])
        .clip(true)
        .style(header_cell_style)
        .into()
}
fn header_cell_with_tooltip<'a>(label: &str, width: f32, tt: &str) -> Element<'a, Message> {
    tooltip(
        header_cell(label, width),
        text(tt.to_string())
            .size(12)
            .color([0.9, 0.9, 0.9])
            .width(Length::Fixed(300.0)),
        tooltip::Position::Top,
    )
    .into()
}

fn body_cell<'a>(value: String, width: f32, zebra: bool) -> Element<'a, Message> {
    let label = text(value.clone()).size(13).wrapping(Wrapping::None);
    let inner = container(label)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding([4, 10])
        .clip(true)
        .style(move |theme: &Theme| body_cell_style(theme, zebra));

    mouse_area(inner).on_press(Message::CopyCell(value)).into()
}

fn row_number_cell<'a>(n: usize, zebra: bool) -> Element<'a, Message> {
    container(text(format!("{}", n)).size(12).wrapping(Wrapping::None))
        .width(Length::Fixed(ROW_NUMBER_WIDTH))
        .height(Length::Fill)
        .padding([4, 10])
        .clip(true)
        .style(move |theme: &Theme| row_number_style(theme, zebra))
        .into()
}

fn view_footer<'a>(
    page: usize,
    page_size_input: &'a str,
    loading: bool,
    total_pages: usize,
) -> Element<'a, Message> {
    let mut prev = button(text("← Prev"));
    if page > 0 && !loading {
        prev = prev.on_press(Message::PrevPage);
    }

    let mut next = button(text("Next →"));
    if page + 1 < total_pages && !loading {
        next = next.on_press(Message::NextPage);
    }

    let label = if total_pages == 0 {
        "Page 0 of 0".to_string()
    } else {
        format!("Page {} of {}", page + 1, total_pages)
    };

    let size_input = text_input("page size", page_size_input)
        .on_input(Message::PageSizeInput)
        .on_submit(Message::PageSizeCommit)
        .width(Length::Fixed(80.0));

    let busy: Element<'_, Message> = if loading {
        text("(loading…)").into()
    } else {
        Space::new().width(Length::Fixed(0.0)).into()
    };

    row![
        prev,
        next,
        text(label),
        Space::new().width(Length::Fixed(24.0)),
        text("Page size:"),
        size_input,
        busy,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

// ---- styling ----

fn header_row_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.strong.color)),
        text_color: Some(p.background.strong.text),
        border: Border {
            color: p.background.strong.color,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn header_cell_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: None,
        text_color: Some(p.background.strong.text),
        border: Border {
            color: p.background.weak.color,
            width: 0.0,
            radius: 0.0.into(),
        },
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
        border: Border {
            color: p.background.weak.color,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn body_cell_style(theme: &Theme, _zebra: bool) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: None,
        text_color: Some(p.background.base.text),
        border: Border {
            color: p.background.strong.color,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn row_number_style(theme: &Theme, zebra: bool) -> ContainerStyle {
    let p = theme.extended_palette();
    let bg = if zebra {
        p.background.weak.color
    } else {
        p.background.base.color
    };
    ContainerStyle {
        background: Some(Background::Color(bg)),
        text_color: Some(p.background.weak.text),
        border: Border {
            color: p.background.strong.color,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..ContainerStyle::default()
    }
}
