use arrow::record_batch::RecordBatch;
use iced::widget::container::Style as ContainerStyle;
use iced::widget::text::Wrapping;
use iced::widget::{
    Space, button, column, container, mouse_area, row, text, text_input, tooltip,
};
use iced::{Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::format::{default_options, row_strings};
use crate::parquet_io::{ColumnStats, FileStats, FileSummary};

const CELL_WIDTH: f32 = 180.0;
const ROW_NUMBER_WIDTH: f32 = 70.0;
const ROW_HEIGHT: f32 = 26.0;
const HEADER_HEIGHT: f32 = 30.0;
const STATS_HEIGHT: f32 = 56.0;
// Beyond this length the value almost certainly overflows CELL_WIDTH and is worth showing in a tooltip.
const OVERFLOW_CHAR_THRESHOLD: usize = 20;

pub fn view<'a>(
    file: &'a FileSummary,
    batch: Option<&'a RecordBatch>,
    page: usize,
    page_size: usize,
    page_size_input: &'a str,
    loading: bool,
    total_pages: usize,
    stats: Option<&'a FileStats>,
    stats_loading: bool,
    stats_error: Option<&'a str>,
) -> Element<'a, Message> {
    let footer = view_footer(page, page_size_input, loading, total_pages);

    let body: Element<'a, Message> = match batch {
        Some(b) if b.num_rows() > 0 => view_grid(b, page, page_size, stats),
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

    let stats_status: Element<'a, Message> = if let Some(err) = stats_error {
        text(format!("Distinct counts unavailable: {err}"))
            .size(12)
            .color(iced::Color::from_rgb(0.9, 0.5, 0.3))
            .into()
    } else if stats_loading {
        text("Computing distinct counts in the background…")
            .size(12)
            .into()
    } else {
        Space::new().width(Length::Fixed(0.0)).into()
    };

    let summary = text(format!(
        "{} rows · {} columns · page size {} · click any cell to copy",
        file.total_rows,
        file.schema.fields().len(),
        page_size
    ))
    .size(13);

    column![summary, stats_status, footer, body].spacing(8).into()
}

fn view_grid<'a>(
    batch: &'a RecordBatch,
    page: usize,
    page_size: usize,
    stats: Option<&'a FileStats>,
) -> Element<'a, Message> {
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

    let stats_row: Element<'a, Message> = {
        let mut r = row![stats_row_label()].spacing(0);
        for c in 0..schema.fields().len() {
            let col_stats = stats.and_then(|s| s.columns.get(c));
            r = r.push(stats_cell(col_stats, CELL_WIDTH));
        }
        container(r)
            .height(Length::Fixed(STATS_HEIGHT))
            .style(stats_row_style)
            .into()
    };

    let row_offset = page * page_size;
    let mut rows_col = column![header, stats_row].spacing(0);
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
        tooltip_box(tt.to_string()),
        tooltip::Position::Top,
    )
    .into()
}

fn stats_row_label<'a>() -> Element<'a, Message> {
    container(
        text("stats")
            .size(11)
            .wrapping(Wrapping::None),
    )
    .width(Length::Fixed(ROW_NUMBER_WIDTH))
    .height(Length::Fill)
    .padding([4, 10])
    .clip(true)
    .into()
}

fn stats_cell<'a>(stats: Option<&'a ColumnStats>, width: f32) -> Element<'a, Message> {
    let content: Element<'a, Message> = match stats {
        Some(s) => {
            let distinct = match s.distinct_count {
                Some(d) => format_count(d as i64),
                None => "…".to_string(),
            };
            let nulls = match s.null_count {
                Some(n) => format_count(n),
                None => "?".to_string(),
            };
            column![
                text(format!("distinct {distinct}"))
                    .size(11)
                    .wrapping(Wrapping::None),
                text(format!("nulls    {nulls}"))
                    .size(11)
                    .wrapping(Wrapping::None),
                text(format!("total    {}", format_count(s.total_count)))
                    .size(11)
                    .wrapping(Wrapping::None),
            ]
            .spacing(1)
            .into()
        }
        None => text("—").size(11).into(),
    };

    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding([4, 10])
        .clip(true)
        .into()
}

fn format_count(n: i64) -> String {
    let neg = n < 0;
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if neg {
        out.push('-');
    }
    let len = digits.len();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn body_cell<'a>(value: String, width: f32, zebra: bool) -> Element<'a, Message> {
    let label = text(value.clone()).size(13).wrapping(Wrapping::None);
    let inner = container(label)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding([4, 10])
        .clip(true)
        .style(move |theme: &Theme| body_cell_style(theme, zebra));

    let with_tooltip: Element<'a, Message> = if value.chars().count() > OVERFLOW_CHAR_THRESHOLD {
        tooltip(inner, tooltip_box(value.clone()), tooltip::Position::Top).into()
    } else {
        inner.into()
    };

    mouse_area(with_tooltip)
        .on_press(Message::CopyCell(value))
        .into()
}

fn tooltip_box<'a>(content: String) -> Element<'a, Message> {
    container(
        text(content)
            .size(12)
            .style(move |theme: &Theme| text::Style {
                color: Some(theme.extended_palette().background.strong.text),
            }),
    )
    .padding([4, 8])
    .max_width(520.0)
    .style(move |theme: &Theme| ContainerStyle {
        background: Some(theme.extended_palette().background.strong.color.into()),
        border: Border {
            color: theme.extended_palette().background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..ContainerStyle::default()
    })
    .into()
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

fn stats_row_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.weak.color)),
        text_color: Some(p.background.weak.text),
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
