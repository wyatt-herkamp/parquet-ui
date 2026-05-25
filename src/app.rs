use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use arrow::record_batch::RecordBatch;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Element, Length, Task};

use crate::parquet_io::{
    CANCELLED, FileStats, FileSummary, compute_distinct, load_metadata, load_page, quick_stats,
};
use crate::views;

const DEFAULT_PAGE_SIZE: usize = 100;

#[derive(Debug, Default)]
pub struct App {
    pub file: Option<FileSummary>,
    pub tab: Tab,
    pub error: Option<String>,
    pub loading: bool,

    pub selected_row_group: Option<usize>,

    pub page: usize,
    pub page_size: usize,
    pub page_size_input: String,
    pub current_batch: Option<RecordBatch>,
    pub page_loading: bool,

    pub stats: Option<FileStats>,
    pub stats_loading: bool,
    pub stats_error: Option<String>,
    pub stats_cancel: Arc<AtomicBool>,

    pub copy_notice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Overview,
    Schema,
    RowGroups,
    Data,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Schema => "Schema",
            Tab::RowGroups => "Row Groups",
            Tab::Data => "Data",
        }
    }

    const ALL: [Tab; 4] = [Tab::Overview, Tab::Schema, Tab::RowGroups, Tab::Data];
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFilePressed,
    FilePicked(Option<PathBuf>),
    FileLoaded(Result<FileSummary, String>),
    TabSelected(Tab),
    RowGroupToggled(usize),
    NextPage,
    PrevPage,
    PageSizeInput(String),
    PageSizeCommit,
    PageLoaded(Result<RecordBatch, String>),
    DistinctLoaded(Arc<AtomicBool>, Result<Vec<usize>, String>),
    CopyCell(String),
    ClearCopyNotice,
}

impl App {
    pub fn new() -> Self {
        Self {
            page_size: DEFAULT_PAGE_SIZE,
            page_size_input: DEFAULT_PAGE_SIZE.to_string(),
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
                self.loading = true;
                self.error = None;
                Task::perform(load_metadata(path), Message::FileLoaded)
            }
            Message::FileLoaded(Ok(summary)) => {
                self.loading = false;
                self.stats_cancel.store(true, Ordering::Relaxed);
                let cancel = Arc::new(AtomicBool::new(false));
                self.stats_cancel = cancel.clone();
                self.stats = Some(quick_stats(&summary));
                self.stats_error = None;
                self.stats_loading = true;
                let path = summary.path.clone();
                self.file = Some(summary);
                self.selected_row_group = None;
                self.page = 0;
                self.current_batch = None;
                let token = cancel.clone();
                let distinct_task = Task::perform(
                    compute_distinct(path, cancel),
                    move |r| Message::DistinctLoaded(token.clone(), r),
                );
                let page_task = if self.tab == Tab::Data {
                    self.dispatch_page_load()
                } else {
                    Task::none()
                };
                Task::batch([distinct_task, page_task])
            }
            Message::FileLoaded(Err(e)) => {
                self.loading = false;
                self.error = Some(e);
                Task::none()
            }
            Message::TabSelected(tab) => {
                self.tab = tab;
                if tab == Tab::Data
                    && self.current_batch.is_none()
                    && self.file.is_some()
                    && !self.page_loading
                {
                    self.dispatch_page_load()
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
            Message::NextPage => {
                if self.page + 1 < self.total_pages() {
                    self.page += 1;
                    self.dispatch_page_load()
                } else {
                    Task::none()
                }
            }
            Message::PrevPage => {
                if self.page > 0 {
                    self.page -= 1;
                    self.dispatch_page_load()
                } else {
                    Task::none()
                }
            }
            Message::PageSizeInput(s) => {
                self.page_size_input = s;
                Task::none()
            }
            Message::PageSizeCommit => {
                if let Ok(n) = self.page_size_input.trim().parse::<usize>() {
                    let n = n.clamp(1, 100_000);
                    if n != self.page_size {
                        self.page_size = n;
                        self.page = 0;
                        self.page_size_input = n.to_string();
                        return self.dispatch_page_load();
                    }
                }
                self.page_size_input = self.page_size.to_string();
                Task::none()
            }
            Message::PageLoaded(Ok(batch)) => {
                self.page_loading = false;
                self.current_batch = Some(batch);
                Task::none()
            }
            Message::PageLoaded(Err(e)) => {
                self.page_loading = false;
                self.error = Some(e);
                Task::none()
            }
            Message::DistinctLoaded(token, result) => {
                if !Arc::ptr_eq(&token, &self.stats_cancel) {
                    return Task::none();
                }
                self.stats_loading = false;
                match result {
                    Ok(distincts) => {
                        if let Some(stats) = self.stats.as_mut() {
                            for (i, d) in distincts.into_iter().enumerate() {
                                if let Some(col) = stats.columns.get_mut(i) {
                                    col.distinct_count = Some(d);
                                }
                            }
                        }
                        self.stats_error = None;
                    }
                    Err(e) if e == CANCELLED => {}
                    Err(e) => self.stats_error = Some(e),
                }
                Task::none()
            }
            Message::CopyCell(value) => {
                let preview = if value.chars().count() > 60 {
                    let mut p: String = value.chars().take(57).collect();
                    p.push_str("…");
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

    fn dispatch_page_load(&mut self) -> Task<Message> {
        let Some(file) = &self.file else {
            return Task::none();
        };
        self.page_loading = true;
        let path = file.path.clone();
        let offset = self.page * self.page_size;
        let limit = self.page_size;
        Task::perform(load_page(path, offset, limit), Message::PageLoaded)
    }

    pub fn total_pages(&self) -> usize {
        let Some(file) = &self.file else {
            return 0;
        };
        let total = file.total_rows.max(0) as usize;
        if total == 0 {
            return 0;
        }
        total.div_ceil(self.page_size.max(1))
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

        let content: Element<'_, Message> = match self.tab {
            Tab::Overview => views::overview::view(file),
            Tab::Schema => views::schema::view(file),
            Tab::RowGroups => views::row_groups::view(file, self.selected_row_group),
            Tab::Data => views::data::view(
                file,
                self.current_batch.as_ref(),
                self.page,
                self.page_size,
                &self.page_size_input,
                self.page_loading,
                self.total_pages(),
                self.stats.as_ref(),
                self.stats_loading,
                self.stats_error.as_deref(),
            ),
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
