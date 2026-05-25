use std::path::PathBuf;
use std::sync::Arc;

use arrow::record_batch::RecordBatch;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Element, Length, Task};

use crate::parquet_io::{FileSummary, load_metadata};
use crate::views;
use crate::wrangle::WrangleSession;
use crate::wrangle::export::export as wrangle_export;
use crate::wrangle::insights::{ColumnInsight, compute_all as compute_insights};
use crate::wrangle::pipeline::{AggFn, Pipeline, Step};
use crate::wrangle::summary::{ColumnSummary, compute as compute_summary};

const DEFAULT_PAGE_SIZE: usize = 100;

#[derive(Debug, Default)]
pub struct App {
    pub file: Option<FileSummary>,
    pub tab: Tab,
    pub error: Option<String>,
    pub loading: bool,

    pub selected_row_group: Option<usize>,

    pub wrangle: WrangleState,

    pub copy_notice: Option<String>,
}

#[derive(Debug, Default)]
pub struct WrangleState {
    pub session: Option<Arc<WrangleSession>>,
    pub session_loading: bool,
    pub session_error: Option<String>,

    pub page: usize,
    pub page_size: usize,
    pub page_size_input: String,
    pub batch: Option<RecordBatch>,
    pub batch_loading: bool,
    pub total_rows: Option<i64>,

    pub insights: Vec<ColumnInsight>,
    pub insights_loading: bool,
    pub insights_error: Option<String>,

    pub selected_column: Option<usize>,
    pub summary: Option<ColumnSummary>,
    pub summary_loading: bool,
    pub summary_error: Option<String>,

    pub pipeline: Pipeline,
    pub editor: Editor,

    pub selected_step: Option<usize>,
    pub diff_before: Option<RecordBatch>,
    pub diff_loading: bool,

    pub export_in_progress: bool,
    pub export_status: Option<String>,

    pub sql_collapsed: bool,
    pub sidebar_collapsed: bool,
}

#[derive(Debug, Clone, Default)]
pub enum Editor {
    #[default]
    None,
    Sort {
        column: String,
        descending: bool,
        nulls_first: bool,
    },
    Filter {
        predicate: String,
    },
    Drop {
        columns: String,
    },
    Rename {
        from: String,
        to: String,
    },
    Cast {
        column: String,
        target_type: String,
    },
    FillNa {
        column: String,
        value: String,
    },
    DropNa {
        columns: String,
    },
    FindReplace {
        column: String,
        pattern: String,
        replacement: String,
        regex: bool,
    },
    Lowercase {
        column: String,
    },
    Uppercase {
        column: String,
    },
    Strip {
        column: String,
    },
    TextLength {
        column: String,
        new_column: String,
    },
    Round {
        column: String,
        decimals: String,
    },
    Floor {
        column: String,
    },
    Ceiling {
        column: String,
    },
    DropDuplicates {
        columns: String,
    },
    GroupByAggregate {
        keys: String,
        agg_col: String,
        agg_fn: AggFn,
        alias: String,
    },
    FormulaColumn {
        new_column: String,
        expression: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum EditorKind {
    Sort,
    Filter,
    Drop,
    Rename,
    Cast,
    FillNa,
    DropNa,
    FindReplace,
    Lowercase,
    Uppercase,
    Strip,
    TextLength,
    Round,
    Floor,
    Ceiling,
    DropDuplicates,
    GroupByAggregate,
    FormulaColumn,
}

#[derive(Debug, Clone, Copy)]
pub enum EditorField {
    Column,
    Descending,
    NullsFirst,
    Predicate,
    Columns,
    From,
    To,
    TargetType,
    Value,
    Pattern,
    Replacement,
    Regex,
    Decimals,
    NewColumn,
    Keys,
    AggCol,
    Alias,
    Expression,
}

impl Editor {
    pub fn default_for(kind: EditorKind) -> Editor {
        match kind {
            EditorKind::Sort => Editor::Sort {
                column: String::new(),
                descending: false,
                nulls_first: false,
            },
            EditorKind::Filter => Editor::Filter {
                predicate: String::new(),
            },
            EditorKind::Drop => Editor::Drop {
                columns: String::new(),
            },
            EditorKind::Rename => Editor::Rename {
                from: String::new(),
                to: String::new(),
            },
            EditorKind::Cast => Editor::Cast {
                column: String::new(),
                target_type: String::new(),
            },
            EditorKind::FillNa => Editor::FillNa {
                column: String::new(),
                value: String::new(),
            },
            EditorKind::DropNa => Editor::DropNa {
                columns: String::new(),
            },
            EditorKind::FindReplace => Editor::FindReplace {
                column: String::new(),
                pattern: String::new(),
                replacement: String::new(),
                regex: false,
            },
            EditorKind::Lowercase => Editor::Lowercase {
                column: String::new(),
            },
            EditorKind::Uppercase => Editor::Uppercase {
                column: String::new(),
            },
            EditorKind::Strip => Editor::Strip {
                column: String::new(),
            },
            EditorKind::TextLength => Editor::TextLength {
                column: String::new(),
                new_column: String::new(),
            },
            EditorKind::Round => Editor::Round {
                column: String::new(),
                decimals: "2".into(),
            },
            EditorKind::Floor => Editor::Floor {
                column: String::new(),
            },
            EditorKind::Ceiling => Editor::Ceiling {
                column: String::new(),
            },
            EditorKind::DropDuplicates => Editor::DropDuplicates {
                columns: String::new(),
            },
            EditorKind::GroupByAggregate => Editor::GroupByAggregate {
                keys: String::new(),
                agg_col: String::new(),
                agg_fn: AggFn::Count,
                alias: String::new(),
            },
            EditorKind::FormulaColumn => Editor::FormulaColumn {
                new_column: String::new(),
                expression: String::new(),
            },
        }
    }

    pub fn set_text(&mut self, field: EditorField, value: String) {
        match (self, field) {
            (Editor::Sort { column, .. }, EditorField::Column) => *column = value,
            (Editor::Filter { predicate }, EditorField::Predicate) => *predicate = value,
            (Editor::Drop { columns }, EditorField::Columns) => *columns = value,
            (Editor::Rename { from, .. }, EditorField::From) => *from = value,
            (Editor::Rename { to, .. }, EditorField::To) => *to = value,
            (Editor::Cast { column, .. }, EditorField::Column) => *column = value,
            (Editor::Cast { target_type, .. }, EditorField::TargetType) => *target_type = value,
            (Editor::FillNa { column, .. }, EditorField::Column) => *column = value,
            (Editor::FillNa { value: v, .. }, EditorField::Value) => *v = value,
            (Editor::DropNa { columns }, EditorField::Columns) => *columns = value,
            (Editor::FindReplace { column, .. }, EditorField::Column) => *column = value,
            (Editor::FindReplace { pattern, .. }, EditorField::Pattern) => *pattern = value,
            (Editor::FindReplace { replacement, .. }, EditorField::Replacement) => {
                *replacement = value
            }
            (Editor::Lowercase { column }, EditorField::Column) => *column = value,
            (Editor::Uppercase { column }, EditorField::Column) => *column = value,
            (Editor::Strip { column }, EditorField::Column) => *column = value,
            (Editor::TextLength { column, .. }, EditorField::Column) => *column = value,
            (Editor::TextLength { new_column, .. }, EditorField::NewColumn) => *new_column = value,
            (Editor::Round { column, .. }, EditorField::Column) => *column = value,
            (Editor::Round { decimals, .. }, EditorField::Decimals) => *decimals = value,
            (Editor::Floor { column }, EditorField::Column) => *column = value,
            (Editor::Ceiling { column }, EditorField::Column) => *column = value,
            (Editor::DropDuplicates { columns }, EditorField::Columns) => *columns = value,
            (Editor::GroupByAggregate { keys, .. }, EditorField::Keys) => *keys = value,
            (Editor::GroupByAggregate { agg_col, .. }, EditorField::AggCol) => *agg_col = value,
            (Editor::GroupByAggregate { alias, .. }, EditorField::Alias) => *alias = value,
            (Editor::FormulaColumn { new_column, .. }, EditorField::NewColumn) => {
                *new_column = value
            }
            (Editor::FormulaColumn { expression, .. }, EditorField::Expression) => {
                *expression = value
            }
            _ => {}
        }
    }

    pub fn set_bool(&mut self, field: EditorField, value: bool) {
        match (self, field) {
            (Editor::Sort { descending, .. }, EditorField::Descending) => *descending = value,
            (Editor::Sort { nulls_first, .. }, EditorField::NullsFirst) => *nulls_first = value,
            (Editor::FindReplace { regex, .. }, EditorField::Regex) => *regex = value,
            _ => {}
        }
    }

    pub fn into_step(self) -> Option<Step> {
        match self {
            Editor::None => None,
            Editor::Sort {
                column,
                descending,
                nulls_first,
            } => valid_required(&column).then_some(Step::Sort {
                column,
                descending,
                nulls_first,
            }),
            Editor::Filter { predicate } => {
                valid_required(&predicate).then_some(Step::Filter { predicate })
            }
            Editor::Drop { columns } => {
                let cols = split_csv(&columns);
                (!cols.is_empty()).then_some(Step::DropColumns { columns: cols })
            }
            Editor::Rename { from, to } => {
                (valid_required(&from) && valid_required(&to)).then_some(Step::Rename { from, to })
            }
            Editor::Cast {
                column,
                target_type,
            } => (valid_required(&column) && valid_required(&target_type)).then_some(Step::Cast {
                column,
                target_type,
            }),
            Editor::FillNa { column, value } => (valid_required(&column) && valid_required(&value))
                .then_some(Step::FillNa { column, value }),
            Editor::DropNa { columns } => Some(Step::DropNa {
                columns: split_csv(&columns),
            }),
            Editor::FindReplace {
                column,
                pattern,
                replacement,
                regex,
            } => {
                (valid_required(&column) && valid_required(&pattern)).then_some(Step::FindReplace {
                    column,
                    pattern,
                    replacement,
                    regex,
                })
            }
            Editor::Lowercase { column } => {
                valid_required(&column).then_some(Step::Lowercase { column })
            }
            Editor::Uppercase { column } => {
                valid_required(&column).then_some(Step::Uppercase { column })
            }
            Editor::Strip { column } => valid_required(&column).then_some(Step::Strip { column }),
            Editor::TextLength { column, new_column } => (valid_required(&column)
                && valid_required(&new_column))
            .then_some(Step::TextLength { column, new_column }),
            Editor::Round { column, decimals } => {
                let d = decimals.trim().parse::<i32>().ok()?;
                valid_required(&column).then_some(Step::Round {
                    column,
                    decimals: d,
                })
            }
            Editor::Floor { column } => valid_required(&column).then_some(Step::Floor { column }),
            Editor::Ceiling { column } => {
                valid_required(&column).then_some(Step::Ceiling { column })
            }
            Editor::DropDuplicates { columns } => Some(Step::DropDuplicates {
                columns: split_csv(&columns),
            }),
            Editor::GroupByAggregate {
                keys,
                agg_col,
                agg_fn,
                alias,
            } => {
                let key_list = split_csv(&keys);
                if !valid_required(&agg_col) || !valid_required(&alias) {
                    return None;
                }
                Some(Step::GroupByAggregate {
                    keys: key_list,
                    aggregations: vec![(agg_col, agg_fn, alias)],
                })
            }
            Editor::FormulaColumn {
                new_column,
                expression,
            } => (valid_required(&new_column) && valid_required(&expression)).then_some({
                Step::FormulaColumn {
                    new_column,
                    expression,
                }
            }),
        }
    }
}

fn valid_required(s: &str) -> bool {
    !s.trim().is_empty()
}

fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Overview,
    RowGroups,
    Data,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::RowGroups => "Row Groups",
            Tab::Data => "Data",
        }
    }

    const ALL: [Tab; 3] = [Tab::Overview, Tab::RowGroups, Tab::Data];
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFilePressed,
    FilePicked(Option<PathBuf>),
    FileLoaded(Result<FileSummary, String>),
    TabSelected(Tab),
    RowGroupToggled(usize),
    WrangleSessionLoaded(Result<Arc<WrangleSession>, String>),
    WrangleRowCountLoaded(Result<i64, String>),
    WranglePageLoaded(Result<RecordBatch, String>),
    WrangleInsightsLoaded(Result<Vec<ColumnInsight>, String>),
    WrangleColumnSelected(usize),
    WrangleSummaryLoaded(usize, Result<ColumnSummary, String>),
    WrangleEditorOpen(EditorKind),
    WrangleEditorCancel,
    WrangleEditorText(EditorField, String),
    WrangleEditorBool(EditorField, bool),
    WrangleEditorAggFn(AggFn),
    WrangleEditorCommit,
    WrangleStepDelete(usize),
    WrangleStepMoveUp(usize),
    WrangleStepMoveDown(usize),
    WrangleStepSelect(Option<usize>),
    WrangleDiffBeforeLoaded(Result<RecordBatch, String>),
    WranglePipelineClear,
    WrangleExportPressed,
    WrangleExportPathPicked(Option<PathBuf>),
    WrangleExportCompleted(Result<PathBuf, String>),
    WrangleSqlToggle,
    WrangleSidebarToggle,
    WrangleNextPage,
    WranglePrevPage,
    WranglePageSizeInput(String),
    WranglePageSizeCommit,
    CopyCell(String),
    ClearCopyNotice,
}

impl App {
    pub fn new() -> Self {
        Self {
            wrangle: WrangleState {
                page_size: DEFAULT_PAGE_SIZE,
                page_size_input: DEFAULT_PAGE_SIZE.to_string(),
                sql_collapsed: true,
                ..WrangleState::default()
            },
            ..Self::default()
        }
    }

    pub fn boot() -> (Self, Task<Message>) {
        let app = Self::new();
        let initial = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .map(|p| Task::done(Message::FilePicked(Some(p))))
            .unwrap_or_else(Task::none);
        (app, initial)
    }

    pub fn title(&self) -> String {
        match &self.file {
            Some(f) => format!(
                "Parquet UI — {}",
                f.path.file_name().and_then(|s| s.to_str()).unwrap_or("?")
            ),
            None => "Parquet UI".to_string(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenFilePressed => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("Parquet", &["parquet"])
                        .pick_file()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::FilePicked,
            ),
            Message::FilePicked(None) => Task::none(),
            Message::FilePicked(Some(path)) => {
                tracing::info!(path = %path.display(), "opening file");
                self.loading = true;
                self.error = None;
                Task::perform(load_metadata(path), Message::FileLoaded)
            }
            Message::FileLoaded(Ok(summary)) => {
                self.loading = false;
                let path = summary.path.clone();
                self.file = Some(summary);
                self.selected_row_group = None;

                // Reset wrangle state for the new file and start opening a session.
                self.wrangle.session = None;
                self.wrangle.session_error = None;
                self.wrangle.session_loading = true;
                self.wrangle.page = 0;
                self.wrangle.batch = None;
                self.wrangle.total_rows = None;
                self.wrangle.insights = Vec::new();
                self.wrangle.insights_loading = false;
                self.wrangle.insights_error = None;
                self.wrangle.selected_column = None;
                self.wrangle.summary = None;
                self.wrangle.summary_loading = false;
                self.wrangle.summary_error = None;
                self.wrangle.pipeline = Pipeline::default();
                self.wrangle.editor = Editor::None;
                self.wrangle.selected_step = None;
                self.wrangle.diff_before = None;
                self.wrangle.diff_loading = false;
                Task::perform(WrangleSession::open(path), Message::WrangleSessionLoaded)
            }
            Message::FileLoaded(Err(e)) => {
                tracing::error!(error = %e, "file load failed");
                self.loading = false;
                self.error = Some(e);
                Task::none()
            }
            Message::TabSelected(tab) => {
                self.tab = tab;
                if tab == Tab::Data
                    && self.wrangle.batch.is_none()
                    && self.wrangle.session.is_some()
                    && !self.wrangle.batch_loading
                {
                    self.dispatch_wrangle_page_load()
                } else {
                    Task::none()
                }
            }
            Message::RowGroupToggled(i) => {
                self.selected_row_group = if self.selected_row_group == Some(i) {
                    None
                } else {
                    Some(i)
                };
                Task::none()
            }
            Message::WrangleSessionLoaded(Ok(session)) => {
                self.wrangle.session_loading = false;
                self.wrangle.session = Some(session.clone());
                self.wrangle.insights_loading = true;
                self.wrangle.insights_error = None;
                let count_task =
                    Task::perform(session.clone().count_rows(), Message::WrangleRowCountLoaded);
                let insights_task = Task::perform(
                    compute_insights(session.clone()),
                    Message::WrangleInsightsLoaded,
                );
                let page_task = if self.tab == Tab::Data {
                    self.dispatch_wrangle_page_load()
                } else {
                    Task::none()
                };
                Task::batch([count_task, insights_task, page_task])
            }
            Message::WrangleInsightsLoaded(Ok(insights)) => {
                self.wrangle.insights_loading = false;
                self.wrangle.insights = insights;
                Task::none()
            }
            Message::WrangleInsightsLoaded(Err(e)) => {
                tracing::warn!(error = %e, "insights load failed");
                self.wrangle.insights_loading = false;
                self.wrangle.insights_error = Some(e);
                Task::none()
            }
            Message::WrangleColumnSelected(idx) => {
                if self.wrangle.selected_column == Some(idx) {
                    // Toggle off
                    self.wrangle.selected_column = None;
                    self.wrangle.summary = None;
                    self.wrangle.summary_loading = false;
                    self.wrangle.summary_error = None;
                    return Task::none();
                }
                let Some(session) = self.wrangle.session.clone() else {
                    return Task::none();
                };
                self.wrangle.selected_column = Some(idx);
                self.wrangle.summary = None;
                self.wrangle.summary_loading = true;
                self.wrangle.summary_error = None;
                Task::perform(compute_summary(session, idx), move |r| {
                    Message::WrangleSummaryLoaded(idx, r)
                })
            }
            Message::WrangleSummaryLoaded(idx, result) => {
                if self.wrangle.selected_column != Some(idx) {
                    return Task::none();
                }
                self.wrangle.summary_loading = false;
                match result {
                    Ok(s) => {
                        self.wrangle.summary = Some(s);
                        self.wrangle.summary_error = None;
                    }
                    Err(e) => self.wrangle.summary_error = Some(e),
                }
                Task::none()
            }
            Message::WrangleSessionLoaded(Err(e)) => {
                tracing::error!(error = %e, "wrangle session open failed");
                self.wrangle.session_loading = false;
                self.wrangle.session_error = Some(e);
                Task::none()
            }
            Message::WrangleRowCountLoaded(Ok(n)) => {
                self.wrangle.total_rows = Some(n);
                Task::none()
            }
            Message::WrangleRowCountLoaded(Err(e)) => {
                self.wrangle.session_error = Some(e);
                Task::none()
            }
            Message::WranglePageLoaded(Ok(batch)) => {
                self.wrangle.batch_loading = false;
                self.wrangle.batch = Some(batch);
                Task::none()
            }
            Message::WranglePageLoaded(Err(e)) => {
                tracing::warn!(error = %e, "page load failed");
                self.wrangle.batch_loading = false;
                self.wrangle.session_error = Some(e);
                Task::none()
            }
            Message::WrangleNextPage => {
                if self.wrangle.page + 1 < self.wrangle_total_pages() {
                    self.wrangle.page += 1;
                    let page = self.dispatch_wrangle_page_load();
                    let diff = self.dispatch_diff_before();
                    Task::batch([page, diff])
                } else {
                    Task::none()
                }
            }
            Message::WranglePrevPage => {
                if self.wrangle.page > 0 {
                    self.wrangle.page -= 1;
                    let page = self.dispatch_wrangle_page_load();
                    let diff = self.dispatch_diff_before();
                    Task::batch([page, diff])
                } else {
                    Task::none()
                }
            }
            Message::WranglePageSizeInput(s) => {
                self.wrangle.page_size_input = s;
                Task::none()
            }
            Message::WranglePageSizeCommit => {
                if let Ok(n) = self.wrangle.page_size_input.trim().parse::<usize>() {
                    let n = n.clamp(1, 100_000);
                    if n != self.wrangle.page_size {
                        self.wrangle.page_size = n;
                        self.wrangle.page = 0;
                        self.wrangle.page_size_input = n.to_string();
                        return self.dispatch_wrangle_page_load();
                    }
                }
                self.wrangle.page_size_input = self.wrangle.page_size.to_string();
                Task::none()
            }
            Message::WrangleEditorOpen(kind) => {
                self.wrangle.editor = Editor::default_for(kind);
                Task::none()
            }
            Message::WrangleEditorCancel => {
                self.wrangle.editor = Editor::None;
                Task::none()
            }
            Message::WrangleEditorText(field, value) => {
                self.wrangle.editor.set_text(field, value);
                Task::none()
            }
            Message::WrangleEditorBool(field, value) => {
                self.wrangle.editor.set_bool(field, value);
                Task::none()
            }
            Message::WrangleEditorAggFn(f) => {
                if let Editor::GroupByAggregate { agg_fn, .. } = &mut self.wrangle.editor {
                    *agg_fn = f;
                }
                Task::none()
            }
            Message::WrangleEditorCommit => {
                let editor = std::mem::take(&mut self.wrangle.editor);
                match editor.into_step() {
                    Some(step) => {
                        self.wrangle.pipeline.push(step);
                        self.dispatch_pipeline_refresh()
                    }
                    None => {
                        // Invalid form; restore (it was already taken). Re-open empty form? Just close.
                        Task::none()
                    }
                }
            }
            Message::WrangleStepDelete(idx) => {
                self.wrangle.pipeline.remove(idx);
                self.dispatch_pipeline_refresh()
            }
            Message::WrangleStepMoveUp(idx) => {
                self.wrangle.pipeline.move_up(idx);
                self.dispatch_pipeline_refresh()
            }
            Message::WrangleStepMoveDown(idx) => {
                self.wrangle.pipeline.move_down(idx);
                self.dispatch_pipeline_refresh()
            }
            Message::WranglePipelineClear => {
                self.wrangle.pipeline = Pipeline::default();
                self.dispatch_pipeline_refresh()
            }
            Message::WrangleStepSelect(maybe_idx) => {
                if self.wrangle.selected_step == maybe_idx {
                    self.wrangle.selected_step = None;
                    self.wrangle.diff_before = None;
                    self.wrangle.diff_loading = false;
                    self.wrangle.page = 0;
                    return Task::batch([
                        self.dispatch_wrangle_page_load(),
                        self.dispatch_wrangle_recount(),
                    ]);
                }
                self.wrangle.selected_step = maybe_idx;
                self.wrangle.page = 0;
                let page = self.dispatch_wrangle_page_load();
                let count = self.dispatch_wrangle_recount();
                let diff = self.dispatch_diff_before();
                Task::batch([page, count, diff])
            }
            Message::WrangleDiffBeforeLoaded(Ok(batch)) => {
                self.wrangle.diff_loading = false;
                self.wrangle.diff_before = Some(batch);
                Task::none()
            }
            Message::WrangleDiffBeforeLoaded(Err(e)) => {
                self.wrangle.diff_loading = false;
                self.wrangle.session_error = Some(e);
                Task::none()
            }
            Message::WrangleExportPressed => {
                if self.wrangle.session.is_none() {
                    return Task::none();
                }
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Parquet", &["parquet"])
                            .add_filter("CSV", &["csv"])
                            .add_filter("JSON Lines", &["json", "jsonl", "ndjson"])
                            .set_file_name("wrangled.parquet")
                            .save_file()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Message::WrangleExportPathPicked,
                )
            }
            Message::WrangleExportPathPicked(None) => Task::none(),
            Message::WrangleExportPathPicked(Some(path)) => {
                let Some(session) = self.wrangle.session.clone() else {
                    return Task::none();
                };
                self.wrangle.export_in_progress = true;
                self.wrangle.export_status = Some(format!("Writing {}…", path.display()));
                let sql = self.wrangle.pipeline.to_sql();
                Task::perform(
                    wrangle_export(session, sql, path),
                    Message::WrangleExportCompleted,
                )
            }
            Message::WrangleExportCompleted(result) => {
                self.wrangle.export_in_progress = false;
                self.wrangle.export_status = Some(match result {
                    Ok(path) => {
                        tracing::info!(dest = %path.display(), "export succeeded");
                        format!("Exported to {}", path.display())
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "export failed");
                        format!("Export failed: {e}")
                    }
                });
                Task::none()
            }
            Message::WrangleSqlToggle => {
                self.wrangle.sql_collapsed = !self.wrangle.sql_collapsed;
                Task::none()
            }
            Message::WrangleSidebarToggle => {
                self.wrangle.sidebar_collapsed = !self.wrangle.sidebar_collapsed;
                Task::none()
            }
            Message::CopyCell(value) => {
                let preview = if value.chars().count() > 60 {
                    let mut p: String = value.chars().take(57).collect();
                    p.push('…');
                    p
                } else {
                    value.clone()
                };
                self.copy_notice = Some(format!("Copied: {preview}"));
                let clear = Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
                    },
                    |_| Message::ClearCopyNotice,
                );
                Task::batch([iced::clipboard::write::<Message>(value), clear])
            }
            Message::ClearCopyNotice => {
                self.copy_notice = None;
                Task::none()
            }
        }
    }

    fn current_pipeline_sql(&self) -> String {
        match self.wrangle.selected_step {
            Some(i) => self.wrangle.pipeline.prefix_sql(i + 1),
            None => self.wrangle.pipeline.to_sql(),
        }
    }

    fn dispatch_wrangle_page_load(&mut self) -> Task<Message> {
        let Some(session) = self.wrangle.session.clone() else {
            return Task::none();
        };
        self.wrangle.batch_loading = true;
        let offset = self.wrangle.page * self.wrangle.page_size;
        let limit = self.wrangle.page_size;
        let sql = self.current_pipeline_sql();
        Task::perform(
            async move {
                session
                    .collect_sql_page(sql, offset, limit)
                    .await
                    .map(|(batch, _schema)| batch)
            },
            Message::WranglePageLoaded,
        )
    }

    fn dispatch_wrangle_recount(&mut self) -> Task<Message> {
        let Some(session) = self.wrangle.session.clone() else {
            return Task::none();
        };
        let sql = self.current_pipeline_sql();
        Task::perform(session.count_sql(sql), Message::WrangleRowCountLoaded)
    }

    fn dispatch_diff_before(&mut self) -> Task<Message> {
        let Some(session) = self.wrangle.session.clone() else {
            return Task::none();
        };
        let Some(step_idx) = self.wrangle.selected_step else {
            self.wrangle.diff_before = None;
            return Task::none();
        };
        self.wrangle.diff_loading = true;
        let offset = self.wrangle.page * self.wrangle.page_size;
        let limit = self.wrangle.page_size;
        let sql = self.wrangle.pipeline.prefix_sql(step_idx);
        Task::perform(
            async move {
                session
                    .collect_sql_page(sql, offset, limit)
                    .await
                    .map(|(batch, _schema)| batch)
            },
            Message::WrangleDiffBeforeLoaded,
        )
    }

    fn dispatch_pipeline_refresh(&mut self) -> Task<Message> {
        self.wrangle.page = 0;
        self.wrangle.selected_step = None;
        self.wrangle.diff_before = None;
        self.wrangle.diff_loading = false;
        let page = self.dispatch_wrangle_page_load();
        let count = self.dispatch_wrangle_recount();
        Task::batch([page, count])
    }

    pub fn wrangle_total_pages(&self) -> usize {
        let Some(n) = self.wrangle.total_rows else {
            return 0;
        };
        let total = n.max(0) as usize;
        if total == 0 {
            return 0;
        }
        total.div_ceil(self.wrangle.page_size.max(1))
    }

    pub fn view(&self) -> Element<'_, Message> {
        let header = self.view_header();
        let tabs = self.view_tabs();
        let body = self.view_body();

        let mut col = column![header, tabs];
        if let Some(err) = &self.error {
            col = col.push(
                container(
                    text(format!("Error: {err}")).color(iced::Color::from_rgb(0.9, 0.3, 0.3)),
                )
                .padding(8),
            );
        }
        col = col.push(body);
        col.spacing(0).into()
    }

    fn view_header(&self) -> Element<'_, Message> {
        let mut open = button(text("Open Parquet…")).on_press(Message::OpenFilePressed);
        if self.loading {
            open = button(text("Loading…"));
        }

        let path_label: Element<'_, Message> = match &self.file {
            Some(f) => text(f.path.display().to_string()).into(),
            None => text("No file loaded").into(),
        };

        let notice: Element<'_, Message> = match &self.copy_notice {
            Some(msg) => container(text(msg.clone()).size(14))
                .padding([4, 10])
                .style(container::rounded_box)
                .into(),
            None => Space::new().width(Length::Fixed(0.0)).into(),
        };

        container(
            row![
                open,
                Space::new().width(Length::Fixed(12.0)),
                path_label,
                Space::new().width(Length::Fill),
                notice,
            ]
            .width(Length::Fill)
            .align_y(iced::Alignment::Center)
            .spacing(8),
        )
        .padding(10)
        .width(Length::Fill)
        .into()
    }

    fn view_tabs(&self) -> Element<'_, Message> {
        let mut r = row![].spacing(4);
        let enabled = self.file.is_some();
        for tab in Tab::ALL {
            let label = text(tab.label());
            let mut btn = button(label);
            if enabled || tab == Tab::Overview {
                btn = btn.on_press(Message::TabSelected(tab));
            }
            if tab == self.tab {
                btn = btn.style(button::primary);
            } else {
                btn = btn.style(button::secondary);
            }
            r = r.push(btn);
        }
        container(r).padding([4, 10]).into()
    }

    fn view_body(&self) -> Element<'_, Message> {
        let Some(file) = &self.file else {
            return container(text("Click \"Open Parquet…\" to choose a file.").size(16))
                .padding(20)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        };

        // The Data tab manages its own scrolling so it can pin the SQL panel to the
        // bottom of the viewport instead of having it scroll with the content.
        if self.tab == Tab::Data {
            return container(views::data::view(&self.wrangle, self.wrangle_total_pages()))
                .padding(12)
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }

        let content: Element<'_, Message> = match self.tab {
            Tab::Overview => views::overview::view(file),
            Tab::RowGroups => views::row_groups::view(file, self.selected_row_group),
            Tab::Data => unreachable!("handled above"),
        };

        scrollable(container(content).padding(12))
            .direction(iced::widget::scrollable::Direction::Both {
                vertical: iced::widget::scrollable::Scrollbar::default(),
                horizontal: iced::widget::scrollable::Scrollbar::default(),
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
