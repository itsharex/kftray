mod file_explorer;
mod navigation;
mod popup;

use std::collections::HashSet;
use std::io;
use std::sync::{
    Arc,
    Mutex,
};

use crossterm::event::{
    self,
    Event,
    KeyCode,
    KeyModifiers,
};
use crossterm::terminal::size;
pub use file_explorer::*;
use kftray_commons::models::{
    config_model::Config,
    config_state_model::ConfigState,
};
pub use popup::*;
use ratatui::widgets::TableState;
use ratatui_explorer::{
    FileExplorer,
    Theme,
};

use crate::core::logging::LOGGER;
use crate::core::port_forward::stop_all_port_forward_and_exit;
use crate::tui::input::navigation::handle_port_forward;

#[derive(PartialEq, Clone, Copy)]
pub enum DeleteButton {
    Confirm,
    Close,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ActiveComponent {
    Menu,
    StoppedTable,
    RunningTable,
    Details,
    Logs,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ActiveTable {
    Stopped,
    Running,
}

#[derive(PartialEq)]
pub enum AppState {
    Normal,
    ShowErrorPopup,
    ShowConfirmationPopup,
    ImportFileExplorerOpen,
    ExportFileExplorerOpen,
    ShowInputPrompt,
    ShowHelp,
    ShowAbout,
    ShowDeleteConfirmation,
}

pub struct App {
    pub details_scroll_offset: usize,
    pub details_scroll_max_offset: usize,
    pub selected_rows_stopped: HashSet<usize>,
    pub selected_rows_running: HashSet<usize>,
    pub import_file_explorer: FileExplorer,
    pub export_file_explorer: FileExplorer,
    pub state: AppState,
    pub selected_row_stopped: usize,
    pub selected_row_running: usize,
    pub active_table: ActiveTable,
    pub import_export_message: Option<String>,
    pub input_buffer: String,
    pub selected_file_path: Option<std::path::PathBuf>,
    pub file_content: Option<String>,
    pub stopped_configs: Vec<Config>,
    pub running_configs: Vec<Config>,
    pub stdout_output: Arc<Mutex<String>>,
    pub error_message: Option<String>,
    pub log_scroll_offset: usize,
    pub log_scroll_max_offset: usize,
    pub active_component: ActiveComponent,
    pub selected_menu_item: usize,
    pub delete_confirmation_message: Option<String>,
    pub selected_delete_button: DeleteButton,
    pub visible_rows: usize,
    pub table_state_stopped: TableState,
    pub table_state_running: TableState,
    pub log_content: Arc<Mutex<String>>,
}

impl App {
    pub fn new() -> Self {
        let theme = Theme::default().add_default_title();
        let import_file_explorer = FileExplorer::with_theme(theme.clone()).unwrap();
        let export_file_explorer = FileExplorer::with_theme(theme).unwrap();
        let stdout_output = LOGGER.buffer.clone();

        let mut app = Self {
            details_scroll_offset: 0,
            details_scroll_max_offset: 0,
            import_file_explorer,
            export_file_explorer,
            state: AppState::Normal,
            selected_row_stopped: 0,
            selected_row_running: 0,
            active_table: ActiveTable::Stopped,
            selected_rows_stopped: HashSet::new(),
            selected_rows_running: HashSet::new(),
            import_export_message: None,
            input_buffer: String::new(),
            selected_file_path: None,
            file_content: None,
            stopped_configs: Vec::new(),
            running_configs: Vec::new(),
            stdout_output,
            error_message: None,
            log_scroll_offset: 0,
            log_scroll_max_offset: 0,
            active_component: ActiveComponent::StoppedTable,
            selected_menu_item: 0,
            delete_confirmation_message: None,
            selected_delete_button: DeleteButton::Confirm,
            visible_rows: 0,
            table_state_stopped: TableState::default(),
            table_state_running: TableState::default(),
            log_content: Arc::new(Mutex::new(String::new())),
        };

        if let Ok((_, height)) = size() {
            app.update_visible_rows(height);
        }

        app
    }

    pub fn update_visible_rows(&mut self, terminal_height: u16) {
        self.visible_rows = (terminal_height.saturating_sub(19)) as usize;
    }

    pub fn update_configs(&mut self, configs: &[Config], config_states: &[ConfigState]) {
        self.stopped_configs = configs
            .iter()
            .filter(|config| {
                config_states
                    .iter()
                    .find(|state| state.config_id == config.id.unwrap_or_default())
                    .map(|state| !state.is_running)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        self.running_configs = configs
            .iter()
            .filter(|config| {
                config_states
                    .iter()
                    .find(|state| state.config_id == config.id.unwrap_or_default())
                    .map(|state| state.is_running)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
    }

    pub fn scroll_up(&mut self) {
        match self.active_table {
            ActiveTable::Stopped => {
                if !self.stopped_configs.is_empty() {
                    if let Some(selected) = self.table_state_stopped.selected() {
                        if selected > 0 {
                            self.table_state_stopped.select(Some(selected - 1));
                            self.selected_row_stopped = selected - 1;
                        }
                    }
                }
            }
            ActiveTable::Running => {
                if !self.running_configs.is_empty() {
                    if let Some(selected) = self.table_state_running.selected() {
                        if selected > 0 {
                            self.table_state_running.select(Some(selected - 1));
                            self.selected_row_running = selected - 1;
                        }
                    }
                }
            }
        }
    }

    pub fn scroll_down(&mut self) {
        match self.active_table {
            ActiveTable::Stopped => {
                if !self.stopped_configs.is_empty() {
                    if let Some(selected) = self.table_state_stopped.selected() {
                        if selected < self.stopped_configs.len() - 1 {
                            self.table_state_stopped.select(Some(selected + 1));
                            self.selected_row_stopped = selected + 1;
                        }
                    } else {
                        self.table_state_stopped.select(Some(0));
                        self.selected_row_stopped = 0;
                    }
                }
            }
            ActiveTable::Running => {
                if !self.running_configs.is_empty() {
                    if let Some(selected) = self.table_state_running.selected() {
                        if selected < self.running_configs.len() - 1 {
                            self.table_state_running.select(Some(selected + 1));
                            self.selected_row_running = selected + 1;
                        }
                    } else {
                        self.table_state_running.select(Some(0));
                        self.selected_row_running = 0;
                    }
                }
            }
        }
    }
}

fn toggle_select_all(app: &mut App) {
    let (selected_rows, configs) = match app.active_table {
        ActiveTable::Stopped => (&mut app.selected_rows_stopped, &app.stopped_configs),
        ActiveTable::Running => (&mut app.selected_rows_running, &app.running_configs),
    };

    if selected_rows.len() == configs.len() {
        selected_rows.clear();
    } else {
        selected_rows.clear();
        for i in 0..configs.len() {
            selected_rows.insert(i);
        }
    }
}

pub async fn handle_input(app: &mut App, _config_states: &mut [ConfigState]) -> io::Result<bool> {
    if event::poll(std::time::Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            log::debug!("Key pressed: {:?}", key);

            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                stop_all_port_forward_and_exit(app).await;
            }

            match app.state {
                AppState::ShowErrorPopup => {
                    log::debug!("Handling ShowErrorPopup state");
                    handle_error_popup_input(app, key.code)?;
                }
                AppState::ShowConfirmationPopup => {
                    log::debug!("Handling ShowConfirmationPopup state");
                    handle_confirmation_popup_input(app, key.code).await?;
                }
                AppState::ImportFileExplorerOpen => {
                    log::debug!("Handling ImportFileExplorerOpen state");
                    handle_import_file_explorer_input(app, key.code).await?;
                }
                AppState::ExportFileExplorerOpen => {
                    log::debug!("Handling ExportFileExplorerOpen state");
                    handle_export_file_explorer_input(app, key.code).await?;
                }
                AppState::ShowInputPrompt => {
                    log::debug!("Handling ShowInputPrompt state");
                    handle_export_input_prompt(app, key.code).await?;
                }
                AppState::ShowHelp => {
                    log::debug!("Handling ShowHelp state");
                    handle_help_input(app, key.code)?;
                }
                AppState::ShowAbout => {
                    log::debug!("Handling ShowAbout state");
                    handle_about_input(app, key.code)?;
                }
                AppState::ShowDeleteConfirmation => {
                    log::debug!("Handling ShowDeleteConfirmation state");
                    handle_delete_confirmation_input(app, key.code).await?;
                }
                AppState::Normal => {
                    log::debug!("Handling Normal state");
                    handle_normal_input(app, key.code).await?;
                }
            }
        } else if let Event::Resize(_, height) = event::read()? {
            log::debug!("Handling Resize event");
            app.update_visible_rows(height);
        }
    }
    Ok(false)
}

async fn handle_normal_input(app: &mut App, key: KeyCode) -> io::Result<()> {
    if handle_common_hotkeys(app, key).await? {
        return Ok(());
    }

    match key {
        KeyCode::Tab => {
            app.active_component = match app.active_component {
                ActiveComponent::Menu => ActiveComponent::StoppedTable,
                ActiveComponent::StoppedTable => ActiveComponent::Details,
                ActiveComponent::Details => ActiveComponent::Menu,
                _ => ActiveComponent::Menu,
            };

            app.active_table = match app.active_component {
                ActiveComponent::StoppedTable => ActiveTable::Stopped,
                _ => app.active_table,
            };
        }
        KeyCode::PageUp => {
            scroll_page_up(app);
        }
        KeyCode::PageDown => {
            scroll_page_down(app);
        }
        _ => match app.active_component {
            ActiveComponent::Menu => handle_menu_input(app, key).await?,
            ActiveComponent::StoppedTable => handle_stopped_table_input(app, key).await?,
            ActiveComponent::RunningTable => handle_running_table_input(app, key).await?,
            ActiveComponent::Details => handle_details_input(app, key).await?,
            ActiveComponent::Logs => handle_logs_input(app, key).await?,
        },
    }
    Ok(())
}

fn scroll_page_up(app: &mut App) {
    match app.active_component {
        ActiveComponent::StoppedTable => {
            let rows_to_scroll = app.visible_rows;
            if app.selected_row_stopped >= rows_to_scroll {
                app.selected_row_stopped -= rows_to_scroll;
            } else {
                app.selected_row_stopped = 0;
            }
            app.table_state_stopped
                .select(Some(app.selected_row_stopped));
        }
        ActiveComponent::RunningTable => {
            let rows_to_scroll = app.visible_rows;
            if app.selected_row_running >= rows_to_scroll {
                app.selected_row_running -= rows_to_scroll;
            } else {
                app.selected_row_running = 0;
            }
            app.table_state_running
                .select(Some(app.selected_row_running));
        }
        ActiveComponent::Logs => {
            if app.log_scroll_offset >= app.visible_rows {
                app.log_scroll_offset -= app.visible_rows;
            } else {
                app.log_scroll_offset = 0;
            }
        }
        ActiveComponent::Details => {
            if app.details_scroll_offset >= app.visible_rows {
                app.details_scroll_offset -= app.visible_rows;
            } else {
                app.details_scroll_offset = 0;
            }
        }
        _ => {}
    }
}

fn scroll_page_down(app: &mut App) {
    match app.active_component {
        ActiveComponent::StoppedTable => {
            let rows_to_scroll = app.visible_rows;
            if app.selected_row_stopped + rows_to_scroll < app.stopped_configs.len() {
                app.selected_row_stopped += rows_to_scroll;
            } else {
                app.selected_row_stopped = app.stopped_configs.len() - 1;
            }
            app.table_state_stopped
                .select(Some(app.selected_row_stopped));
        }
        ActiveComponent::RunningTable => {
            let rows_to_scroll = app.visible_rows;
            if app.selected_row_running + rows_to_scroll < app.running_configs.len() {
                app.selected_row_running += rows_to_scroll;
            } else {
                app.selected_row_running = app.running_configs.len() - 1;
            }
            app.table_state_running
                .select(Some(app.selected_row_running));
        }
        ActiveComponent::Logs => {
            if app.log_scroll_offset + app.visible_rows < app.log_scroll_max_offset {
                app.log_scroll_offset += app.visible_rows;
            } else {
                app.log_scroll_offset = app.log_scroll_max_offset;
            }
        }
        ActiveComponent::Details => {
            if app.details_scroll_offset + app.visible_rows < app.details_scroll_max_offset {
                app.details_scroll_offset += app.visible_rows;
            } else {
                app.details_scroll_offset = app.details_scroll_max_offset;
            }
        }
        _ => {}
    }
}

fn select_first_row(app: &mut App) {
    match app.active_table {
        ActiveTable::Stopped => {
            if !app.stopped_configs.is_empty() {
                app.table_state_stopped.select(Some(0));
            }
        }
        ActiveTable::Running => {
            if !app.running_configs.is_empty() {
                app.table_state_running.select(Some(0));
            }
        }
    }
}

fn clear_selection(app: &mut App) {
    match app.active_table {
        ActiveTable::Stopped => {
            app.selected_rows_running.clear();
            app.table_state_running.select(None);
        }
        ActiveTable::Running => {
            app.selected_rows_stopped.clear();
            app.table_state_stopped.select(None);
        }
    }
}

async fn handle_menu_input(app: &mut App, key: KeyCode) -> io::Result<()> {
    if handle_common_hotkeys(app, key).await? {
        return Ok(());
    }

    match key {
        KeyCode::Left => {
            if app.selected_menu_item > 0 {
                app.selected_menu_item -= 1
            }
        }
        KeyCode::Right => {
            if app.selected_menu_item < 4 {
                app.selected_menu_item += 1
            }
        }
        KeyCode::Down => {
            app.active_component = ActiveComponent::StoppedTable;
            app.active_table = ActiveTable::Stopped;
            clear_selection(app);
            select_first_row(app);
        }
        KeyCode::Enter => match app.selected_menu_item {
            0 => app.state = AppState::ShowHelp,
            1 => open_import_file_explorer(app),
            2 => open_export_file_explorer(app),
            3 => app.state = AppState::ShowAbout,
            4 => stop_all_port_forward_and_exit(app).await,
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

async fn handle_stopped_table_input(app: &mut App, key: KeyCode) -> io::Result<()> {
    if handle_common_hotkeys(app, key).await? {
        return Ok(());
    }

    match key {
        KeyCode::Right => {
            app.active_component = ActiveComponent::RunningTable;
            app.active_table = ActiveTable::Running;
            clear_selection(app);
            select_first_row(app);
        }
        KeyCode::Up => {
            if app.table_state_stopped.selected() == Some(0) {
                app.active_component = ActiveComponent::Menu;
                app.table_state_running.select(None);
                app.selected_rows_stopped.clear();
                app.table_state_stopped.select(None);
            } else {
                app.scroll_up();
            }
        }
        KeyCode::Down => {
            if app.stopped_configs.is_empty()
                || app.table_state_stopped.selected() == Some(app.stopped_configs.len() - 1)
            {
                app.active_component = ActiveComponent::Details;
                app.table_state_running.select(None);
                app.selected_rows_stopped.clear();
                app.table_state_stopped.select(None);
            } else {
                app.scroll_down();
            }
        }
        KeyCode::Char(' ') => toggle_row_selection(app),
        KeyCode::Char('f') => handle_port_forwarding(app).await?,
        KeyCode::Char('d') => show_delete_confirmation(app),
        KeyCode::Char('a') => toggle_select_all(app),
        _ => {}
    }
    Ok(())
}

async fn handle_running_table_input(app: &mut App, key: KeyCode) -> io::Result<()> {
    if handle_common_hotkeys(app, key).await? {
        return Ok(());
    }

    match key {
        KeyCode::Left => {
            app.active_component = ActiveComponent::StoppedTable;
            app.active_table = ActiveTable::Stopped;
            clear_selection(app);
            select_first_row(app);
        }
        KeyCode::Up => {
            if app.running_configs.is_empty() || app.table_state_running.selected() == Some(0) {
                app.active_component = ActiveComponent::Menu;
                app.table_state_running.select(None);
                app.selected_rows_stopped.clear();
                app.table_state_stopped.select(None);
            } else {
                app.scroll_up();
            }
        }
        KeyCode::Down => {
            if app.running_configs.is_empty()
                || app.table_state_running.selected() == Some(app.running_configs.len() - 1)
            {
                app.active_component = ActiveComponent::Logs;
                app.table_state_running.select(None);
                app.selected_rows_stopped.clear();
                app.table_state_stopped.select(None);
            } else {
                app.scroll_down();
            }
        }
        KeyCode::Char(' ') => toggle_row_selection(app),
        KeyCode::Char('f') => handle_port_forwarding(app).await?,
        KeyCode::Char('d') => show_delete_confirmation(app),
        KeyCode::Char('a') => toggle_select_all(app),
        _ => {}
    }
    Ok(())
}

async fn handle_details_input(app: &mut App, key: KeyCode) -> io::Result<()> {
    if handle_common_hotkeys(app, key).await? {
        return Ok(());
    }

    match key {
        KeyCode::Right => app.active_component = ActiveComponent::Logs,
        KeyCode::Up => {
            app.active_component = ActiveComponent::StoppedTable;
            app.active_table = ActiveTable::Stopped;
            clear_selection(app);
            select_first_row(app);
        }
        KeyCode::PageUp => {
            scroll_page_up(app);
        }
        KeyCode::PageDown => {
            scroll_page_down(app);
        }
        _ => {}
    }
    Ok(())
}

async fn handle_logs_input(app: &mut App, key: KeyCode) -> io::Result<()> {
    if handle_common_hotkeys(app, key).await? {
        return Ok(());
    }

    match key {
        KeyCode::Left => app.active_component = ActiveComponent::Details,
        KeyCode::Up => {
            app.active_component = ActiveComponent::RunningTable;
            app.active_table = ActiveTable::Running;
            clear_selection(app);
            select_first_row(app);
        }
        KeyCode::PageUp => {
            scroll_page_up(app);
        }
        KeyCode::PageDown => {
            scroll_page_down(app);
        }
        _ => {}
    }
    Ok(())
}

async fn handle_common_hotkeys(app: &mut App, key: KeyCode) -> io::Result<bool> {
    match key {
        KeyCode::Char('c') => {
            clear_stdout_output(app);
            Ok(true)
        }
        KeyCode::Char('q') => {
            app.state = AppState::ShowAbout;
            Ok(true)
        }
        KeyCode::Char('i') => {
            open_import_file_explorer(app);
            Ok(true)
        }
        KeyCode::Char('e') => {
            open_export_file_explorer(app);
            Ok(true)
        }
        KeyCode::Char('h') => {
            app.state = AppState::ShowHelp;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn toggle_row_selection(app: &mut App) {
    let (selected_row, selected_rows) = match app.active_table {
        ActiveTable::Stopped => (
            app.table_state_stopped.selected().unwrap_or(0),
            &mut app.selected_rows_stopped,
        ),
        ActiveTable::Running => (
            app.table_state_running.selected().unwrap_or(0),
            &mut app.selected_rows_running,
        ),
    };

    if selected_rows.contains(&selected_row) {
        selected_rows.remove(&selected_row);
    } else {
        selected_rows.insert(selected_row);
    }
}

fn clear_stdout_output(app: &mut App) {
    let mut stdout_output = app.stdout_output.lock().unwrap();
    stdout_output.clear();
}

async fn handle_port_forwarding(app: &mut App) -> io::Result<()> {
    let (selected_rows, configs, selected_row) = match app.active_table {
        ActiveTable::Stopped => (
            &mut app.selected_rows_stopped,
            &app.stopped_configs,
            app.selected_row_stopped,
        ),
        ActiveTable::Running => (
            &mut app.selected_rows_running,
            &app.running_configs,
            app.selected_row_running,
        ),
    };

    if configs.is_empty() {
        return Ok(());
    }

    if selected_rows.is_empty() {
        selected_rows.insert(selected_row);
    }

    let selected_configs: Vec<Config> = selected_rows
        .iter()
        .filter_map(|&row| configs.get(row).cloned())
        .collect();

    for config in selected_configs.clone() {
        handle_port_forward(app, config).await;
    }

    if app.active_table == ActiveTable::Stopped {
        app.running_configs.extend(selected_configs.clone());
        app.stopped_configs
            .retain(|config| !selected_configs.contains(config));
    } else {
        app.stopped_configs.extend(selected_configs.clone());
        app.running_configs
            .retain(|config| !selected_configs.contains(config));
    }

    match app.active_table {
        ActiveTable::Stopped => app.selected_rows_stopped.clear(),
        ActiveTable::Running => app.selected_rows_running.clear(),
    }

    Ok(())
}

fn show_delete_confirmation(app: &mut App) {
    if !app.selected_rows_stopped.is_empty() {
        app.state = AppState::ShowDeleteConfirmation;
        app.delete_confirmation_message =
            Some("Are you sure you want to delete the selected configs?".to_string());
    }
}

async fn handle_delete_confirmation_input(app: &mut App, key: KeyCode) -> io::Result<()> {
    match key {
        KeyCode::Left | KeyCode::Right => {
            app.selected_delete_button = match app.selected_delete_button {
                DeleteButton::Confirm => DeleteButton::Close,
                DeleteButton::Close => DeleteButton::Confirm,
            };
        }
        KeyCode::Enter => {
            if app.selected_delete_button == DeleteButton::Confirm {
                let ids_to_delete: Vec<i64> = app
                    .selected_rows_stopped
                    .iter()
                    .filter_map(|&row| app.stopped_configs.get(row).and_then(|config| config.id))
                    .collect();

                match kftray_commons::utils::config::delete_configs(ids_to_delete.clone()).await {
                    Ok(_) => {
                        app.delete_confirmation_message =
                            Some("Configs deleted successfully.".to_string());
                        app.stopped_configs.retain(|config| {
                            !ids_to_delete.contains(&config.id.unwrap_or_default())
                        });
                    }
                    Err(e) => {
                        app.delete_confirmation_message =
                            Some(format!("Failed to delete configs: {}", e));
                    }
                }
            }
            app.selected_rows_stopped.clear();
            app.state = AppState::Normal;
        }
        KeyCode::Esc => app.state = AppState::Normal,
        _ => {}
    }
    Ok(())
}

fn open_import_file_explorer(app: &mut App) {
    app.state = AppState::ImportFileExplorerOpen;
    app.selected_file_path = std::env::current_dir().ok();
}

fn open_export_file_explorer(app: &mut App) {
    app.state = AppState::ExportFileExplorerOpen;
    app.selected_file_path = std::env::current_dir().ok();
}