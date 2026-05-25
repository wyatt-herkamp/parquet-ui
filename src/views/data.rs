use arrow::record_batch::RecordBatch;
use iced::widget::container::Style as ContainerStyle;
use iced::widget::text::Wrapping;
use iced::widget::{
    Space, button, canvas, column, container, mouse_area, row, scrollable, text, text_input,
    tooltip,
};
use iced::{Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::app::{Editor, EditorField, EditorKind, Message, WrangleState};
use crate::format::{default_options, row_strings};
use crate::wrangle::insights::{ColumnInsight, Histogram};
use crate::wrangle::pipeline::{AggFn, Pipeline};
use crate::wrangle::summary::ColumnSummary;

const CELL_WIDTH: f32 = 180.0;
const ROW_NUMBER_WIDTH: f32 = 70.0;
const ROW_HEIGHT: f32 = 26.0;
const HEADER_HEIGHT: f32 = 30.0;
const INSIGHTS_HEIGHT: f32 = 100.0;
const HISTO_HEIGHT: f32 = 36.0;
const OVERFLOW_CHAR_THRESHOLD: usize = 20;

pub fn view<'a>(state: &'a WrangleState, total_pages: usize) -> Element<'a, Message> {
    if state.session_loading && state.session.is_none() {
        return container(text("Opening DataFusion session…").size(14))
            .padding(12)
            .into();
    }

    if let Some(err) = &state.session_error {
        return container(text(format!("Wrangle error: {err}")).size(14))
            .padding(12)
            .into();
    }

    let footer = view_footer(
        state.page,
        &state.page_size_input,
        state.batch_loading,
        total_pages,
    );

    let body: Element<'a, Message> = match state.batch.as_ref() {
        Some(b) if b.num_rows() > 0 => view_grid(
            b,
            state.page,
            state.page_size,
            &state.insights,
            state.selected_column,
            state.diff_before.as_ref(),
        ),
        Some(_) => container(text("(no rows on this page)")).padding(12).into(),
        None => {
            let msg = if state.batch_loading {
                "Loading…"
            } else {
                "No data loaded yet."
            };
            container(text(msg)).padding(12).into()
        }
    };

    let summary_panel: Element<'a, Message> = summary_panel(state);
    let diff_banner: Element<'a, Message> = diff_banner(state);
    let sql_panel: Element<'a, Message> = sql_panel(&state.pipeline, state.sql_collapsed);

    let total_rows = state
        .total_rows
        .map(|n| n.to_string())
        .unwrap_or_else(|| "…".into());
    let cols = state
        .session
        .as_ref()
        .map(|s| s.schema.fields().len())
        .unwrap_or(0);
    let summary = text(format!(
        "{total_rows} rows · {cols} columns · page size {} · DataFusion engine",
        state.page_size
    ))
    .size(13);

    let status: Element<'a, Message> = if let Some(err) = &state.insights_error {
        text(format!("Insights unavailable: {err}"))
            .size(12)
            .color(Color::from_rgb(0.9, 0.5, 0.3))
            .into()
    } else if state.insights_loading {
        text("Computing column insights…").size(12).into()
    } else {
        Space::new().width(Length::Fixed(0.0)).into()
    };

    let top_bar = row![
        column![summary, status].spacing(2),
        Space::new().width(Length::Fill),
        sidebar_toggle_button(state.sidebar_collapsed),
    ]
    .align_y(iced::Alignment::Center);

    let sidebar: Element<'a, Message> = if state.sidebar_collapsed {
        collapsed_sidebar()
    } else {
        expanded_sidebar(state)
    };

    let main_col: Element<'a, Message> = column![diff_banner, summary_panel, footer, body]
        .spacing(8)
        .into();

    // Main grid + right sidebar scroll together; SQL panel below is pinned to the bottom of the viewport.
    let split = row![
        scrollable(container(main_col).padding([0, 8]))
            .direction(iced::widget::scrollable::Direction::Both {
                vertical: iced::widget::scrollable::Scrollbar::default(),
                horizontal: iced::widget::scrollable::Scrollbar::default(),
            })
            .width(Length::Fill)
            .height(Length::Fill),
        sidebar,
    ]
    .spacing(0);

    column![top_bar, split, sql_panel]
        .spacing(10)
        .height(Length::Fill)
        .into()
}

fn sidebar_toggle_button<'a>(collapsed: bool) -> Element<'a, Message> {
    let label = if collapsed {
        "▸  Show transforms"
    } else {
        "◂  Hide transforms"
    };
    button(text(label).size(12))
        .style(button::secondary)
        .on_press(Message::WrangleSidebarToggle)
        .into()
}

fn collapsed_sidebar<'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fixed(0.0))
        .height(Length::Fixed(0.0))
        .into()
}

fn expanded_sidebar<'a>(state: &'a WrangleState) -> Element<'a, Message> {
    let toolbar_panel = toolbar(state);
    let editor_panel = editor_form(&state.editor);
    let steps_panel_w = steps_panel(&state.pipeline, state.selected_step);
    let export_status_w = export_status_line(state);

    let inner = column![
        text("Transformations").size(14),
        toolbar_panel,
        export_status_w,
        editor_panel,
        steps_panel_w,
    ]
    .spacing(10);

    container(scrollable(container(inner).padding([0, 12])).height(Length::Fill))
        .width(Length::Fixed(380.0))
        .height(Length::Fill)
        .style(sidebar_panel_style)
        .into()
}

fn sql_panel<'a>(pipeline: &'a Pipeline, collapsed: bool) -> Element<'a, Message> {
    let sql = pipeline.to_pretty_sql();
    let toggle_label = if collapsed { "Show SQL" } else { "Hide SQL" };
    let header = row![
        text("SQL (DataFusion)").size(13),
        Space::new().width(Length::Fill),
        button(text(toggle_label).size(11))
            .style(button::secondary)
            .on_press(Message::WrangleSqlToggle),
        button(text("Copy SQL").size(11))
            .style(button::secondary)
            .on_press(Message::CopyCell(sql.clone())),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let body: Element<'a, Message> = if collapsed {
        Space::new().width(Length::Fixed(0.0)).into()
    } else {
        container(text(sql).size(11).wrapping(Wrapping::Word))
            .padding(8)
            .width(Length::Fill)
            .style(sql_body_style)
            .into()
    };

    container(column![header, body].spacing(6))
        .padding(10)
        .width(Length::Fill)
        .style(editor_panel_style)
        .into()
}

fn diff_banner<'a>(state: &'a WrangleState) -> Element<'a, Message> {
    let Some(idx) = state.selected_step else {
        return Space::new().width(Length::Fixed(0.0)).into();
    };
    let desc = state
        .pipeline
        .steps
        .get(idx)
        .map(|s| s.description())
        .unwrap_or_else(|| format!("step {idx}"));
    let status_text = if state.diff_loading {
        " · diffing…".to_string()
    } else if state.diff_before.is_some() {
        " · changed cells highlighted in yellow".to_string()
    } else {
        String::new()
    };
    container(
        row![
            text(format!(
                "Viewing pipeline through step {n}: {desc}{status_text}",
                n = idx + 1
            ))
            .size(12),
            Space::new().width(Length::Fill),
            button(text("Exit diff").size(11))
                .style(button::secondary)
                .on_press(Message::WrangleStepSelect(Some(idx))),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 10])
    .width(Length::Fill)
    .style(diff_banner_style)
    .into()
}

fn toolbar<'a>(_state: &'a WrangleState) -> Element<'a, Message> {
    let categories: [(&str, &[(EditorKind, &str)]); 6] = [
        (
            "Columns",
            &[
                (EditorKind::Drop, "Drop"),
                (EditorKind::Rename, "Rename"),
                (EditorKind::Cast, "Cast type"),
                (EditorKind::TextLength, "Length →"),
                (EditorKind::FormulaColumn, "Formula"),
            ],
        ),
        (
            "Rows",
            &[
                (EditorKind::Sort, "Sort"),
                (EditorKind::Filter, "Filter"),
                (EditorKind::DropNa, "Drop nulls"),
                (EditorKind::DropDuplicates, "Distinct"),
            ],
        ),
        (
            "Values",
            &[
                (EditorKind::FillNa, "Fill nulls"),
                (EditorKind::FindReplace, "Find/Replace"),
            ],
        ),
        (
            "Text",
            &[
                (EditorKind::Lowercase, "lower"),
                (EditorKind::Uppercase, "UPPER"),
                (EditorKind::Strip, "Strip"),
            ],
        ),
        (
            "Math",
            &[
                (EditorKind::Round, "Round"),
                (EditorKind::Floor, "Floor"),
                (EditorKind::Ceiling, "Ceil"),
            ],
        ),
        ("Aggregate", &[(EditorKind::GroupByAggregate, "Group by")]),
    ];

    const PER_ROW: usize = 4;
    let mut groups = column![].spacing(4);
    for (label, items) in categories {
        for (i, chunk) in items.chunks(PER_ROW).enumerate() {
            let label_widget: Element<'_, Message> = if i == 0 {
                category_label(label)
            } else {
                category_label("")
            };
            let mut row_w = row![label_widget]
                .spacing(4)
                .align_y(iced::Alignment::Center);
            for (kind, btn_label) in chunk {
                row_w = row_w.push(toolbar_button(*kind, btn_label));
            }
            groups = groups.push(row_w);
        }
    }

    let actions = row![
        Space::new().width(Length::Fill),
        button(text("Clear pipeline").size(11))
            .style(button::danger)
            .on_press(Message::WranglePipelineClear),
        button(text("Export…").size(11))
            .style(button::primary)
            .on_press(Message::WrangleExportPressed),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    container(column![groups, actions].spacing(8))
        .padding(10)
        .width(Length::Fill)
        .style(toolbar_style)
        .into()
}

fn category_label<'a>(label: &str) -> Element<'a, Message> {
    container(
        text(label.to_string())
            .size(11)
            .style(category_label_text_style),
    )
    .width(Length::Fixed(78.0))
    .padding([0, 6])
    .into()
}

fn category_label_text_style(theme: &Theme) -> text::Style {
    let p = theme.extended_palette();
    text::Style {
        color: Some(p.background.base.text),
    }
}

fn toolbar_button<'a>(kind: EditorKind, label: &str) -> Element<'a, Message> {
    button(text(label.to_string()).size(11))
        .style(button::secondary)
        .on_press(Message::WrangleEditorOpen(kind))
        .into()
}

fn export_status_line<'a>(state: &'a WrangleState) -> Element<'a, Message> {
    if state.export_in_progress {
        return text(state.export_status.as_deref().unwrap_or("Exporting…"))
            .size(11)
            .into();
    }
    match &state.export_status {
        Some(msg) => text(msg).size(11).into(),
        None => Space::new().width(Length::Fixed(0.0)).into(),
    }
}

fn editor_form<'a>(editor: &'a Editor) -> Element<'a, Message> {
    let (title, hint, body): (&'static str, &'static str, Element<'a, Message>) = match editor {
        Editor::None => return Space::new().width(Length::Fixed(0.0)).into(),
        Editor::Sort {
            column,
            descending,
            nulls_first,
        } => (
            "Sort rows",
            "Order rows by one column.",
            row![
                labelled(
                    "Column",
                    text_field(column, EditorField::Column, "col_name", 200.0)
                ),
                labelled(
                    "Direction",
                    segmented_bool(
                        EditorField::Descending,
                        *descending,
                        "Ascending",
                        "Descending"
                    ),
                ),
                labelled(
                    "Nulls",
                    segmented_bool(EditorField::NullsFirst, *nulls_first, "Last", "First"),
                ),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .into(),
        ),
        Editor::Filter { predicate } => (
            "Filter rows",
            "Keep rows matching a SQL predicate.",
            labelled(
                "Predicate (SQL)",
                text_field(
                    predicate,
                    EditorField::Predicate,
                    "col > 0 AND status = 'ok'",
                    500.0,
                ),
            ),
        ),
        Editor::Drop { columns } => (
            "Drop columns",
            "Remove one or more columns from the result.",
            labelled(
                "Columns (comma-separated)",
                text_field(columns, EditorField::Columns, "col_a, col_b", 360.0),
            ),
        ),
        Editor::Rename { from, to } => (
            "Rename column",
            "Give a column a different name.",
            row![
                labelled(
                    "From",
                    text_field(from, EditorField::From, "old_name", 200.0)
                ),
                labelled("To", text_field(to, EditorField::To, "new_name", 200.0)),
            ]
            .spacing(12)
            .into(),
        ),
        Editor::Cast {
            column,
            target_type,
        } => (
            "Cast column type",
            "Convert a column's data type.",
            row![
                labelled(
                    "Column",
                    text_field(column, EditorField::Column, "col_name", 200.0)
                ),
                labelled(
                    "Target type",
                    text_field(
                        target_type,
                        EditorField::TargetType,
                        "DOUBLE / VARCHAR / DATE / TIMESTAMP",
                        260.0,
                    ),
                ),
            ]
            .spacing(12)
            .into(),
        ),
        Editor::FillNa { column, value } => (
            "Fill nulls",
            "Replace null values in a column with a SQL literal.",
            row![
                labelled(
                    "Column",
                    text_field(column, EditorField::Column, "col_name", 200.0)
                ),
                labelled(
                    "Value (SQL literal)",
                    text_field(value, EditorField::Value, "0   or   'unknown'", 240.0),
                ),
            ]
            .spacing(12)
            .into(),
        ),
        Editor::DropNa { columns } => (
            "Drop rows with nulls",
            "Remove rows that have nulls in any listed column (blank = any column).",
            labelled(
                "Columns (blank = any column null)",
                text_field(columns, EditorField::Columns, "col_a, col_b", 360.0),
            ),
        ),
        Editor::FindReplace {
            column,
            pattern,
            replacement,
            regex,
        } => (
            "Find and replace",
            "Replace text matches inside a column.",
            column![
                row![
                    labelled(
                        "Column",
                        text_field(column, EditorField::Column, "col_name", 200.0)
                    ),
                    labelled(
                        "Match",
                        segmented_bool(EditorField::Regex, *regex, "Plain text", "Regex"),
                    ),
                ]
                .spacing(12),
                row![
                    labelled(
                        "Find",
                        text_field(pattern, EditorField::Pattern, "old_text", 240.0),
                    ),
                    labelled(
                        "Replace with",
                        text_field(replacement, EditorField::Replacement, "new_text", 240.0),
                    ),
                ]
                .spacing(12),
            ]
            .spacing(8)
            .into(),
        ),
        Editor::Lowercase { column } => (
            "Lowercase",
            "Convert all letters in a column to lowercase.",
            labelled(
                "Column",
                text_field(column, EditorField::Column, "col_name", 240.0),
            ),
        ),
        Editor::Uppercase { column } => (
            "Uppercase",
            "Convert all letters in a column to uppercase.",
            labelled(
                "Column",
                text_field(column, EditorField::Column, "col_name", 240.0),
            ),
        ),
        Editor::Strip { column } => (
            "Strip whitespace",
            "Trim leading/trailing whitespace from string values.",
            labelled(
                "Column",
                text_field(column, EditorField::Column, "col_name", 240.0),
            ),
        ),
        Editor::TextLength { column, new_column } => (
            "Text length → new column",
            "Add a new column that holds the character length of an existing column.",
            row![
                labelled(
                    "Source column",
                    text_field(column, EditorField::Column, "col_name", 200.0)
                ),
                labelled(
                    "New column",
                    text_field(new_column, EditorField::NewColumn, "col_name_len", 200.0),
                ),
            ]
            .spacing(12)
            .into(),
        ),
        Editor::Round { column, decimals } => (
            "Round",
            "Round a numeric column to N decimals.",
            row![
                labelled(
                    "Column",
                    text_field(column, EditorField::Column, "col_name", 200.0)
                ),
                labelled(
                    "Decimals",
                    text_field(decimals, EditorField::Decimals, "2", 80.0),
                ),
            ]
            .spacing(12)
            .into(),
        ),
        Editor::Floor { column } => (
            "Floor",
            "Round a numeric column down to the nearest integer.",
            labelled(
                "Column",
                text_field(column, EditorField::Column, "col_name", 240.0),
            ),
        ),
        Editor::Ceiling { column } => (
            "Ceiling",
            "Round a numeric column up to the nearest integer.",
            labelled(
                "Column",
                text_field(column, EditorField::Column, "col_name", 240.0),
            ),
        ),
        Editor::DropDuplicates { columns } => (
            "Drop duplicate rows",
            "Keep only the first occurrence of each unique row.",
            labelled(
                "Columns (blank = whole row)",
                text_field(columns, EditorField::Columns, "col_a, col_b", 360.0),
            ),
        ),
        Editor::GroupByAggregate {
            keys,
            agg_col,
            agg_fn,
            alias,
        } => (
            "Group by + aggregate",
            "Collapse rows into groups and compute an aggregate. Blank keys = single group.",
            column![
                labelled(
                    "Group keys",
                    text_field(
                        keys,
                        EditorField::Keys,
                        "col_a, col_b   (blank = single group)",
                        420.0
                    ),
                ),
                row![
                    labelled(
                        "Aggregate column",
                        text_field(agg_col, EditorField::AggCol, "col_name", 200.0),
                    ),
                    labelled("Function", agg_fn_picker(*agg_fn)),
                    labelled(
                        "Alias",
                        text_field(alias, EditorField::Alias, "result_name", 200.0)
                    ),
                ]
                .spacing(12),
            ]
            .spacing(8)
            .into(),
        ),
        Editor::FormulaColumn {
            new_column,
            expression,
        } => (
            "Formula column",
            "Add a new column from a SQL expression.",
            row![
                labelled(
                    "New column",
                    text_field(new_column, EditorField::NewColumn, "derived_col", 200.0),
                ),
                labelled(
                    "Expression (SQL)",
                    text_field(
                        expression,
                        EditorField::Expression,
                        "col_a + col_b * 2",
                        420.0
                    ),
                ),
            ]
            .spacing(12)
            .into(),
        ),
    };

    let actions = row![
        Space::new().width(Length::Fill),
        button(text("Cancel").size(12))
            .style(button::secondary)
            .on_press(Message::WrangleEditorCancel),
        button(text("✓  Add step").size(12))
            .style(button::primary)
            .on_press(Message::WrangleEditorCommit),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let header = column![
        text(format!("New step  ·  {title}")).size(14),
        text(hint).size(11),
    ]
    .spacing(2);

    container(column![header, body, actions].spacing(10).padding(0))
        .padding(12)
        .width(Length::Fill)
        .style(editor_panel_style)
        .into()
}

fn agg_fn_picker<'a>(current: AggFn) -> Element<'a, Message> {
    let mut r = row![].spacing(4);
    for f in AggFn::ALL {
        let mut b = button(text(f.label()).size(11));
        b = if matches!(
            (current, f),
            (AggFn::Count, AggFn::Count)
                | (AggFn::CountDistinct, AggFn::CountDistinct)
                | (AggFn::Sum, AggFn::Sum)
                | (AggFn::Avg, AggFn::Avg)
                | (AggFn::Min, AggFn::Min)
                | (AggFn::Max, AggFn::Max)
        ) {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        r = r.push(b.on_press(Message::WrangleEditorAggFn(f)));
    }
    r.into()
}

fn labelled<'a>(label: &'a str, child: Element<'a, Message>) -> Element<'a, Message> {
    column![text(label).size(11), child].spacing(2).into()
}

fn text_field<'a>(
    value: &'a str,
    field: EditorField,
    placeholder: &'a str,
    width: f32,
) -> Element<'a, Message> {
    text_input(placeholder, value)
        .on_input(move |v| Message::WrangleEditorText(field, v))
        .width(Length::Fixed(width))
        .size(12)
        .into()
}

fn segmented_bool<'a>(
    field: EditorField,
    value: bool,
    false_label: &'a str,
    true_label: &'a str,
) -> Element<'a, Message> {
    let off = button(text(false_label.to_string()).size(11))
        .style(if value {
            button::secondary
        } else {
            button::primary
        })
        .on_press(Message::WrangleEditorBool(field, false));
    let on = button(text(true_label.to_string()).size(11))
        .style(if value {
            button::primary
        } else {
            button::secondary
        })
        .on_press(Message::WrangleEditorBool(field, true));
    row![off, on].spacing(2).into()
}

fn steps_panel<'a>(pipeline: &'a Pipeline, selected_step: Option<usize>) -> Element<'a, Message> {
    if pipeline.is_empty() {
        return container(
            row![
                text("📋").size(16),
                text("Cleaning Steps will appear here. Pick a transform above to add one.")
                    .size(12),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding(12)
        .width(Length::Fill)
        .style(empty_state_style)
        .into();
    }
    let count = pipeline.steps.len();
    let header = row![
        text(format!("Cleaning Steps  ·  {count}")).size(13),
        Space::new().width(Length::Fixed(8.0)),
        text("(click any step to inspect its before/after diff)").size(11),
    ]
    .align_y(iced::Alignment::Center);
    let mut col = column![header].spacing(4);
    for (i, step) in pipeline.steps.iter().enumerate() {
        let selected = selected_step == Some(i);

        let badge = container(text(format!("{}", i + 1)).size(11).wrapping(Wrapping::None))
            .width(Length::Fixed(22.0))
            .height(Length::Fixed(22.0))
            .center_x(Length::Fixed(22.0))
            .center_y(Length::Fixed(22.0))
            .style(step_badge_style);

        let mut up = button(text("↑").size(11)).style(button::secondary);
        if i > 0 {
            up = up.on_press(Message::WrangleStepMoveUp(i));
        }
        let mut down = button(text("↓").size(11)).style(button::secondary);
        if i + 1 < count {
            down = down.on_press(Message::WrangleStepMoveDown(i));
        }
        let delete = button(text("✕").size(11))
            .style(button::danger)
            .on_press(Message::WrangleStepDelete(i));

        let label_area = mouse_area(
            container(text(step.description()).size(12).wrapping(Wrapping::None))
                .padding([0, 4])
                .width(Length::Fill)
                .clip(true),
        )
        .on_press(Message::WrangleStepSelect(Some(i)));

        col = col.push(
            container(
                row![badge, label_area, up, down, delete]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
            )
            .padding([6, 10])
            .style(move |theme: &Theme| step_row_style_selected(theme, selected)),
        );
    }
    col.into()
}

fn view_grid<'a>(
    batch: &'a RecordBatch,
    page: usize,
    page_size: usize,
    insights: &'a [ColumnInsight],
    selected_column: Option<usize>,
    diff_before: Option<&'a RecordBatch>,
) -> Element<'a, Message> {
    let opts = default_options();
    let schema = batch.schema();

    let mut header = row![header_cell("#", ROW_NUMBER_WIDTH)].spacing(0);
    for (idx, f) in schema.fields().iter().enumerate() {
        let selected = selected_column == Some(idx);
        header = header.push(clickable_column_header(
            f.name(),
            CELL_WIDTH,
            &format!("{}", f.data_type()),
            idx,
            selected,
        ));
    }
    let header: Element<'a, Message> = container(header)
        .height(Length::Fixed(HEADER_HEIGHT))
        .style(header_row_style)
        .into();

    let before_rows: Option<Vec<Vec<String>>> = diff_before.map(|b| {
        (0..b.num_rows())
            .map(|r| row_strings(b, r, &opts))
            .collect()
    });

    let mut insights_row = row![spacer_cell(ROW_NUMBER_WIDTH, INSIGHTS_HEIGHT)].spacing(0);
    for (c, _f) in schema.fields().iter().enumerate() {
        let ci = insights.get(c);
        insights_row = insights_row.push(insights_cell(ci, CELL_WIDTH));
    }
    let insights_block: Element<'a, Message> = container(insights_row)
        .height(Length::Fixed(INSIGHTS_HEIGHT))
        .style(insights_row_style)
        .into();

    let row_offset = page * page_size;
    let mut rows_col = column![header, insights_block].spacing(0);
    for r in 0..batch.num_rows() {
        let values = row_strings(batch, r, &opts);
        let zebra = r % 2 == 1;
        let mut row_widgets = row![row_number_cell(row_offset + r + 1, zebra)].spacing(0);
        let row_before: Option<&Vec<String>> = before_rows.as_ref().and_then(|b| b.get(r));
        for (c, v) in values.into_iter().enumerate() {
            let changed = row_before
                .and_then(|b| b.get(c))
                .map(|prev| prev != &v)
                .unwrap_or(false);
            row_widgets = row_widgets.push(body_cell(v, CELL_WIDTH, zebra, changed));
        }
        let styled = container(row_widgets)
            .height(Length::Fixed(ROW_HEIGHT))
            .style(move |theme: &Theme| body_row_style(theme, zebra));
        rows_col = rows_col.push(styled);
    }

    rows_col.into()
}

fn spacer_cell<'a>(width: f32, height: f32) -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
}

fn insights_cell<'a>(insight: Option<&'a ColumnInsight>, width: f32) -> Element<'a, Message> {
    let inner: Element<'a, Message> = match insight {
        Some(ci) => {
            let visual: Element<'a, Message> = match (&ci.histogram, &ci.top_values) {
                (Some(h), _) => histogram_widget(h.clone(), width - 16.0, HISTO_HEIGHT),
                (None, Some(top)) if !top.is_empty() => top_values_widget(top, width - 16.0),
                _ => Space::new()
                    .width(Length::Fixed(width - 16.0))
                    .height(Length::Fixed(HISTO_HEIGHT))
                    .into(),
            };

            let null_pct = if ci.total > 0 {
                (ci.null_count as f64) * 100.0 / (ci.total as f64)
            } else {
                0.0
            };
            let distinct_label = match ci.distinct {
                Some(d) => format!("≈{}", format_count(d as i64)),
                None => "—".to_string(),
            };
            let stats_line = text(format!(
                "distinct {distinct_label}   missing {null_pct:.1}%"
            ))
            .size(11)
            .wrapping(Wrapping::None);

            let range_line: Element<'a, Message> = match (&ci.min, &ci.max) {
                (Some(lo), Some(hi)) => text(format!("min {lo} · max {hi}"))
                    .size(10)
                    .wrapping(Wrapping::None)
                    .into(),
                _ => Space::new().width(Length::Fixed(0.0)).into(),
            };

            column![visual, stats_line, range_line].spacing(2).into()
        }
        None => text("…").size(11).into(),
    };

    container(inner)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding([4, 8])
        .clip(true)
        .into()
}

fn histogram_widget<'a>(h: Histogram, width: f32, height: f32) -> Element<'a, Message> {
    canvas(HistoCanvas { h })
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
}

fn top_values_widget<'a>(top: &'a [(String, u64)], width: f32) -> Element<'a, Message> {
    let max = top.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1) as f32;
    let widest_count = top
        .iter()
        .map(|(_, c)| format_count(*c as i64).chars().count())
        .max()
        .unwrap_or(1);
    let count_w = ((widest_count as f32) * 6.5 + 4.0).clamp(28.0, width * 0.35);
    let bar_w = (width * 0.3).max(24.0);
    let label_w = (width - count_w - bar_w - 8.0).max(40.0);
    let mut col = column![].spacing(2);
    for (v, c) in top.iter().take(3) {
        let frac = (*c as f32) / max;
        col = col.push(
            row![
                container(text(elide(v, 14)).size(10).wrapping(Wrapping::None))
                    .width(Length::Fixed(label_w))
                    .clip(true),
                bar(frac, bar_w, 8.0),
                container(
                    text(format_count(*c as i64))
                        .size(10)
                        .wrapping(Wrapping::None)
                )
                .width(Length::Fixed(count_w))
                .clip(true),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(4),
        );
    }
    col.into()
}

fn bar<'a>(frac: f32, width: f32, height: f32) -> Element<'a, Message> {
    let fill = (width * frac.clamp(0.0, 1.0)).max(1.0);
    container(
        container(Space::new())
            .width(Length::Fixed(fill))
            .height(Length::Fixed(height))
            .style(bar_fill_style),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(height))
    .style(bar_bg_style)
    .into()
}

fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
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
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

struct HistoCanvas {
    h: Histogram,
}

impl canvas::Program<Message> for HistoCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let p = theme.extended_palette();
        let bg = p.background.weak.color;
        let fg = p.primary.base.color;

        frame.fill_rectangle(Point::ORIGIN, Size::new(bounds.width, bounds.height), bg);

        let n = self.h.bin_counts.len().max(1);
        let max = self.h.bin_counts.iter().copied().max().unwrap_or(1).max(1) as f32;
        let bar_w = bounds.width / n as f32;
        let pad = 1.0_f32.min(bar_w * 0.2);

        for (i, c) in self.h.bin_counts.iter().enumerate() {
            let h = (*c as f32 / max) * bounds.height;
            if h <= 0.0 {
                continue;
            }
            let x = i as f32 * bar_w + pad * 0.5;
            let y = bounds.height - h;
            frame.fill_rectangle(Point::new(x, y), Size::new((bar_w - pad).max(1.0), h), fg);
        }

        vec![frame.into_geometry()]
    }
}

fn view_footer<'a>(
    page: usize,
    page_size_input: &'a str,
    loading: bool,
    total_pages: usize,
) -> Element<'a, Message> {
    let mut prev = button(text("← Prev"));
    if page > 0 && !loading {
        prev = prev.on_press(Message::WranglePrevPage);
    }
    let mut next = button(text("Next →"));
    if page + 1 < total_pages && !loading {
        next = next.on_press(Message::WrangleNextPage);
    }
    let label = if total_pages == 0 {
        "Page 0 of 0".to_string()
    } else {
        format!("Page {} of {}", page + 1, total_pages)
    };
    let size_input = text_input("page size", page_size_input)
        .on_input(Message::WranglePageSizeInput)
        .on_submit(Message::WranglePageSizeCommit)
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

fn header_cell<'a>(label: &str, width: f32) -> Element<'a, Message> {
    container(text(label.to_string()).size(13).wrapping(Wrapping::None))
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding([6, 10])
        .clip(true)
        .style(header_cell_style)
        .into()
}

fn clickable_column_header<'a>(
    label: &str,
    width: f32,
    tt: &str,
    idx: usize,
    selected: bool,
) -> Element<'a, Message> {
    let cell = container(text(label.to_string()).size(13).wrapping(Wrapping::None))
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding([6, 10])
        .clip(true)
        .style(move |theme: &Theme| selectable_header_style(theme, selected));

    let with_tt = tooltip(cell, tooltip_box(tt.to_string()), tooltip::Position::Top);
    mouse_area(with_tt)
        .on_press(Message::WrangleColumnSelected(idx))
        .into()
}

fn summary_panel<'a>(state: &'a WrangleState) -> Element<'a, Message> {
    let Some(idx) = state.selected_column else {
        return Space::new().width(Length::Fixed(0.0)).into();
    };
    let title = state
        .session
        .as_ref()
        .and_then(|s| s.schema.fields().get(idx).map(|f| f.name().to_string()))
        .unwrap_or_else(|| format!("column {idx}"));

    let body: Element<'a, Message> = if let Some(err) = &state.summary_error {
        text(format!("Summary unavailable: {err}"))
            .size(12)
            .color(Color::from_rgb(0.9, 0.5, 0.3))
            .into()
    } else if state.summary_loading {
        text("Computing summary…").size(12).into()
    } else if let Some(s) = &state.summary {
        summary_body(s)
    } else {
        text("(no summary)").size(12).into()
    };

    container(
        column![
            row![
                text(format!("Summary: {title}")).size(14),
                Space::new().width(Length::Fill),
                button(text("Close").size(11))
                    .style(button::secondary)
                    .on_press(Message::WrangleColumnSelected(idx)),
            ]
            .align_y(iced::Alignment::Center),
            body,
        ]
        .spacing(6),
    )
    .padding(10)
    .width(Length::Fill)
    .style(summary_panel_style)
    .into()
}

fn summary_body<'a>(s: &'a ColumnSummary) -> Element<'a, Message> {
    let null_pct = if s.total > 0 {
        (s.null_count as f64) * 100.0 / (s.total as f64)
    } else {
        0.0
    };
    let distinct = s
        .distinct
        .map(|d| format_count(d as i64))
        .unwrap_or_else(|| "—".into());
    let line1 = text(format!(
        "total {} · nulls {} ({null_pct:.2}%) · distinct ≈{distinct}",
        format_count(s.total as i64),
        format_count(s.null_count as i64),
    ))
    .size(12);

    let mut col = column![line1].spacing(3);
    if let (Some(lo), Some(hi)) = (&s.min, &s.max) {
        col = col.push(text(format!("range: {lo}  →  {hi}")).size(12));
    }
    if let (Some(mean), Some(std)) = (s.mean, s.std) {
        col = col.push(text(format!("mean {mean:.4} · std {std:.4}")).size(12));
    }
    if let (Some(q25), Some(q50), Some(q75)) = (s.q25, s.q50, s.q75) {
        col = col.push(
            text(format!(
                "quartiles: 25% {q25:.4} · 50% {q50:.4} · 75% {q75:.4}"
            ))
            .size(12),
        );
    }
    if !s.top_values.is_empty() {
        col = col.push(text("Top values:").size(12));
        let max = s
            .top_values
            .iter()
            .map(|(_, c)| *c)
            .max()
            .unwrap_or(1)
            .max(1) as f32;
        for (v, c) in &s.top_values {
            let frac = (*c as f32) / max;
            col = col.push(
                row![
                    container(text(elide(v, 28)).size(11).wrapping(Wrapping::None))
                        .width(Length::Fixed(220.0))
                        .clip(true),
                    bar(frac, 200.0, 8.0),
                    container(
                        text(format_count(*c as i64))
                            .size(11)
                            .wrapping(Wrapping::None)
                    )
                    .width(Length::Fixed(80.0))
                    .clip(true),
                ]
                .align_y(iced::Alignment::Center)
                .spacing(8),
            );
        }
    }
    col.into()
}

fn selectable_header_style(theme: &Theme, selected: bool) -> ContainerStyle {
    let p = theme.extended_palette();
    let bg = if selected {
        Some(Background::Color(p.primary.weak.color))
    } else {
        None
    };
    ContainerStyle {
        background: bg,
        text_color: Some(p.background.strong.text),
        border: Border::default(),
        ..ContainerStyle::default()
    }
}

fn summary_panel_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.weak.color)),
        text_color: Some(p.background.weak.text),
        border: Border {
            color: p.background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn body_cell<'a>(value: String, width: f32, zebra: bool, changed: bool) -> Element<'a, Message> {
    let label = text(value.clone()).size(13).wrapping(Wrapping::None);
    let inner = container(label)
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .padding([4, 10])
        .clip(true)
        .style(move |theme: &Theme| body_cell_style(theme, zebra, changed));

    let with_tooltip: Element<'a, Message> = if value.chars().count() > OVERFLOW_CHAR_THRESHOLD {
        tooltip(inner, tooltip_box(value.clone()), tooltip::Position::Top).into()
    } else {
        inner.into()
    };

    mouse_area(with_tooltip)
        .on_press(Message::CopyCell(value))
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

fn sidebar_panel_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.base.color)),
        text_color: Some(p.background.base.text),
        border: Border {
            color: p.background.strong.color,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn toolbar_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.weak.color)),
        text_color: Some(p.background.weak.text),
        border: Border {
            color: p.background.strong.color,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn step_badge_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.primary.base.color)),
        text_color: Some(p.primary.base.text),
        border: Border {
            color: p.primary.base.color,
            width: 0.0,
            radius: 10.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn empty_state_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.weak.color)),
        text_color: Some(p.background.weak.text),
        border: Border {
            color: p.background.strong.color,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn sql_body_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.base.color)),
        text_color: Some(p.background.base.text),
        border: Border {
            color: p.background.strong.color,
            width: 1.0,
            radius: 3.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn editor_panel_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.weak.color)),
        text_color: Some(p.background.weak.text),
        border: Border {
            color: p.primary.base.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn step_row_style_selected(theme: &Theme, selected: bool) -> ContainerStyle {
    let p = theme.extended_palette();
    let bg = if selected {
        p.primary.weak.color
    } else {
        p.background.weak.color
    };
    ContainerStyle {
        background: Some(Background::Color(bg)),
        text_color: Some(p.background.weak.text),
        border: Border {
            color: if selected {
                p.primary.base.color
            } else {
                p.background.strong.color
            },
            width: 1.0,
            radius: 3.0.into(),
        },
        ..ContainerStyle::default()
    }
}

fn insights_row_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.weak.color)),
        text_color: Some(p.background.weak.text),
        border: Border::default(),
        ..ContainerStyle::default()
    }
}

fn bar_bg_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.background.base.color)),
        border: Border::default(),
        ..ContainerStyle::default()
    }
}

fn bar_fill_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.primary.base.color)),
        border: Border::default(),
        ..ContainerStyle::default()
    }
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

fn header_cell_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: None,
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

fn body_cell_style(theme: &Theme, _zebra: bool, changed: bool) -> ContainerStyle {
    let p = theme.extended_palette();
    let bg = if changed {
        Some(Background::Color(Color::from_rgba(0.95, 0.85, 0.2, 0.45)))
    } else {
        None
    };
    ContainerStyle {
        background: bg,
        text_color: Some(p.background.base.text),
        border: Border::default(),
        ..ContainerStyle::default()
    }
}

fn diff_banner_style(theme: &Theme) -> ContainerStyle {
    let p = theme.extended_palette();
    ContainerStyle {
        background: Some(Background::Color(p.primary.weak.color)),
        text_color: Some(p.primary.weak.text),
        border: Border {
            color: p.primary.base.color,
            width: 1.0,
            radius: 4.0.into(),
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
        border: Border::default(),
        ..ContainerStyle::default()
    }
}
