use std::io::{self, stdout};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Alignment, Layout, Constraint, Direction, Rect},
    prelude::{CrosstermBackend, Terminal, Backend, Frame},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap, Table, Row, BarChart},
};
use rand::seq::SliceRandom;
use walkdir::WalkDir;

use crate::models::config::Config;
use crate::services::player_service::{PlayerService, PlayerCommand, PlayerStatus};
use crate::services::playlist_storage_service::{self, Playlist};
use crate::services::metadata_service::PlaylistService;

pub fn run(config: &Config) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(&config.music_path);

    draw_loading_screen(&mut terminal, "loading incredible musical data... please wait a few seconds.")?;
    app.scan_directory();

    app.run(&mut terminal)?;

    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn draw_loading_screen<B: Backend>(terminal: &mut Terminal<B>, message: &str) -> io::Result<()> {
    terminal.draw(|f| {
        let size = f.size();
        let loading_text = Paragraph::new(message)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title("Loading")
                    .borders(Borders::ALL)
            );
        f.render_widget(loading_text, size);
    })?;
    Ok(())
}

struct InputState {
    last_key_press: Instant,
}

impl InputState {
    fn new() -> Self {
        InputState {
            last_key_press: Instant::now(),
        }
    }

    fn can_process_key(&mut self) -> bool {
        if self.last_key_press.elapsed() > Duration::from_millis(100) {
            self.last_key_press = Instant::now();
            return true;
        }
        false
    }
}

#[derive(Clone, Copy)]
enum ActiveTab {
    FolderNavigation,
    PlaylistNavigation,
    Stats,
}

impl From<ActiveTab> for usize {
    fn from(tab: ActiveTab) -> Self {
        match tab {
            ActiveTab::FolderNavigation => 0,
            ActiveTab::PlaylistNavigation => 1,
            ActiveTab::Stats => 2,
        }
    }
}

struct App {
    music_path: PathBuf,
    current_dir: PathBuf,
    items: Vec<String>,
    filtered_items: Vec<String>,
    selected: usize,
    marked_tracks: Vec<PathBuf>,
    playlist_name_input: String,
    is_creating_playlist: bool,
    is_adding_to_playlist: bool,
    playlist_creation_selected: usize,
    is_deleting_playlist: bool,
    playlist_to_delete: Option<usize>,
    playlists: Vec<Playlist>,
    playlist_service: PlaylistService,
    input_state: InputState,
    player: PlayerService,
    current_folder: Option<String>,
    is_playing: bool,
    is_paused: bool,
    is_shuffle_mode: bool,
    active_tab: ActiveTab,
    playlist_selected: usize,
    playlist_track_selected: usize,
    viewing_playlist: Option<usize>,
    volume: f32,
    is_searching: bool,
    search_query: String,
}

impl App {
    fn new(music_path: &str) -> Self {
        let playlist_service = PlaylistService::new();

        let mut app = App {
            music_path: PathBuf::from(music_path),
            current_dir: PathBuf::from(music_path),
            items: vec![],
            filtered_items: vec![],
            selected: 0,
            marked_tracks: vec![],
            playlist_name_input: String::new(),
            is_creating_playlist: false,
            is_adding_to_playlist: false,
            playlist_creation_selected: 0,
            is_deleting_playlist: false,
            playlist_to_delete: None,
            playlists: playlist_storage_service::load_playlists().unwrap_or_default(),
            playlist_service,
            input_state: InputState::new(),
            player: PlayerService::new(),
            current_folder: None,
            is_playing: false,
            is_paused: false,
            is_shuffle_mode: false,
            active_tab: ActiveTab::FolderNavigation,
            playlist_selected: 0,
            playlist_track_selected: 0,
            viewing_playlist: None,
            volume: 1.0,
            is_searching: false,
            search_query: String::new(),
        };
        app.update_items();
        app
    }

    fn scan_directory(&mut self) {
        self.playlist_service.scan_directory(self.music_path.as_path()).unwrap();
    }


    fn update_items(&mut self) {
        self.items = std::fs::read_dir(&self.current_dir)
            .unwrap_or_else(|_| std::fs::read_dir(".").unwrap())
            .filter_map(|res| res.ok())
            .filter(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    return true;
                }
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    return matches!(ext.to_lowercase().as_str(), "mp3" | "flac" | "ogg" | "wav");
                }
                false
            })
            .map(|entry| {
                let file_name = entry.file_name().into_string().unwrap_or_default();
                if entry.path().is_dir() {
                    format!("[DIR] {}", file_name)
                } else {
                    file_name
                }
            })
            .collect();

        if self.current_dir.parent().is_some() {
            self.items.insert(0, "[DIR] ..".to_string());
        }

        self.items.sort();

        if self.selected >= self.items.len() && !self.items.is_empty() {
            self.selected = 0;
        }
        self.update_filtered_items();
    }

    fn update_filtered_items(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_items = self.items.clone();
        } else {
            self.filtered_items = self.items
                .iter()
                .filter(|item| item.to_lowercase().contains(&self.search_query.to_lowercase()))
                .cloned()
                .collect();
        }
        if self.selected >= self.filtered_items.len() && !self.filtered_items.is_empty() {
            self.selected = 0;
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            while let Ok(status) = self.player.receiver.try_recv() {
                match status {
                    PlayerStatus::Volume(vol) => self.volume = vol,
                }
            }

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if self.input_state.can_process_key() {
                        if self.handle_input(key)? {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    fn handle_input(&mut self, key: event::KeyEvent) -> io::Result<bool> {
        if self.is_deleting_playlist {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(index) = self.playlist_to_delete {
                        let playlist = &self.playlists[index];
                        playlist_storage_service::delete_playlist(&playlist.name)?;
                        self.playlists.remove(index);
                    }
                    self.is_deleting_playlist = false;
                    self.playlist_to_delete = None;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.is_deleting_playlist = false;
                    self.playlist_to_delete = None;
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.is_searching {
            match key.code {
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.update_filtered_items();
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.update_filtered_items();
                }
                KeyCode::Enter => {
                    self.is_searching = false;
                }
                KeyCode::Esc => {
                    self.is_searching = false;
                    self.search_query.clear();
                    self.update_filtered_items();
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.is_creating_playlist {
            match key.code {
                KeyCode::Up => {
                    if self.playlist_creation_selected > 0 {
                        self.playlist_creation_selected -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.playlist_creation_selected < self.playlists.len() {
                        self.playlist_creation_selected += 1;
                    }
                }
                KeyCode::Enter => {
                    if self.playlist_creation_selected == self.playlists.len() {
                        // Create new playlist
                        let playlist = Playlist {
                            name: self.playlist_name_input.clone(),
                            tracks: self.marked_tracks.clone(),
                        };
                        playlist_storage_service::save_playlist(&playlist)?;
                        self.playlists.push(playlist);
                    } else {
                        // Add to existing playlist
                        let playlist = &mut self.playlists[self.playlist_creation_selected];
                        playlist.tracks.extend(self.marked_tracks.clone());
                        playlist_storage_service::save_playlist(playlist)?;
                    }
                    self.is_creating_playlist = false;
                    self.playlist_name_input.clear();
                    self.marked_tracks.clear();
                    self.playlist_creation_selected = 0;
                }
                KeyCode::Char(c) => {
                    if self.playlist_creation_selected == self.playlists.len() {
                        self.playlist_name_input.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if self.playlist_creation_selected == self.playlists.len() {
                        self.playlist_name_input.pop();
                    }
                }
                KeyCode::Esc => {
                    self.is_creating_playlist = false;
                    self.playlist_name_input.clear();
                    self.playlist_creation_selected = 0;
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char('1') => self.active_tab = ActiveTab::FolderNavigation,
                KeyCode::Char('2') => self.active_tab = ActiveTab::PlaylistNavigation,
                KeyCode::Char('3') => self.active_tab = ActiveTab::Stats,
                KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
                KeyCode::Up => {
                    match self.active_tab {
                        ActiveTab::FolderNavigation => {
                            let items_len = self.filtered_items.len();
                            if self.selected > 0 {
                                self.selected -= 1;
                            } else if items_len > 0 {
                                self.selected = items_len - 1;
                            }
                        }
                        ActiveTab::PlaylistNavigation => {
                            if let Some(playlist_index) = self.viewing_playlist {
                                let playlist = &self.playlists[playlist_index];
                                if self.playlist_track_selected > 0 {
                                    self.playlist_track_selected -= 1;
                                } else if !playlist.tracks.is_empty() {
                                    self.playlist_track_selected = playlist.tracks.len() - 1;
                                }
                            } else {
                                if self.playlist_selected > 0 {
                                    self.playlist_selected -= 1;
                                } else if !self.playlists.is_empty() {
                                    self.playlist_selected = self.playlists.len() - 1;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                KeyCode::Down => {
                    match self.active_tab {
                        ActiveTab::FolderNavigation => {
                            let items_len = self.filtered_items.len();
                            if self.selected < items_len.saturating_sub(1) {
                                self.selected += 1;
                            } else {
                                self.selected = 0;
                            }
                        }
                        ActiveTab::PlaylistNavigation => {
                            if let Some(playlist_index) = self.viewing_playlist {
                                let playlist = &self.playlists[playlist_index];
                                if self.playlist_track_selected < playlist.tracks.len().saturating_sub(1) {
                                    self.playlist_track_selected += 1;
                                } else {
                                    self.playlist_track_selected = 0;
                                }
                            } else {
                                if self.playlist_selected < self.playlists.len().saturating_sub(1) {
                                    self.playlist_selected += 1;
                                } else {
                                    self.playlist_selected = 0;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    if let ActiveTab::FolderNavigation = self.active_tab {
                        self.is_searching = true;
                        self.search_query.clear();
                    }
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if !self.is_creating_playlist {
                        self.is_creating_playlist = true;
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    if let ActiveTab::PlaylistNavigation = self.active_tab {
                        if self.viewing_playlist.is_none() && !self.playlists.is_empty() {
                            self.is_deleting_playlist = true;
                            self.playlist_to_delete = Some(self.playlist_selected);
                        }
                    }
                }
                KeyCode::Enter => {
                    if !self.filtered_items.is_empty() {
                        match self.active_tab {
                            ActiveTab::FolderNavigation => {
                                let selected_item = self.filtered_items[self.selected].clone();

                                if selected_item == "[DIR] .." {
                                    if self.current_dir.parent().is_some() {
                                        self.current_dir.pop();
                                        self.selected = 0;
                                        self.search_query.clear();
                                        self.is_searching = false;
                                        self.update_items();
                                    }
                                } else if selected_item.starts_with("[DIR] ") {
                                    let dir_name = &selected_item[6..];
                                    let new_path = self.current_dir.join(dir_name);
                                    if new_path.is_dir() {
                                        self.current_dir = new_path;
                                        self.selected = 0;
                                        self.search_query.clear();
                                        self.is_searching = false;
                                        self.update_items();
                                    }
                                } else {
                                    let track_path = self.current_dir.join(selected_item);
                                    if Self::is_audio_file(&track_path) {
                                        self.current_folder = self.current_dir.file_name()
                                            .and_then(|n| n.to_str())
                                            .map(|s| s.to_string());
                                        self.is_playing = true;
                                        self.is_paused = false;
                                        self.is_shuffle_mode = false;
                                        self.player.send(PlayerCommand::PlayAlbum(vec![track_path]));
                                    }
                                }
                            }
                            ActiveTab::PlaylistNavigation => {
                                if let Some(playlist_index) = self.viewing_playlist {
                                    let playlist = &self.playlists[playlist_index];
                                    if self.playlist_track_selected < playlist.tracks.len() {
                                        let track_path = &playlist.tracks[self.playlist_track_selected];
                                        if track_path.exists() && Self::is_audio_file(track_path) {
                                            self.current_folder = Some(format!("Playlist: {}", playlist.name));
                                            self.is_playing = true;
                                            self.is_paused = false;
                                            self.is_shuffle_mode = false;
                                            self.player.send(PlayerCommand::PlayAlbum(vec![track_path.clone()]));
                                        }
                                    }
                                } else if !self.playlists.is_empty() {
                                    self.viewing_playlist = Some(self.playlist_selected);
                                    self.playlist_track_selected = 0;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                KeyCode::Esc => {
                    if self.is_searching {
                        self.is_searching = false;
                        self.search_query.clear();
                        self.update_filtered_items();
                    } else {
                        match self.active_tab {
                            ActiveTab::FolderNavigation => {
                                if self.current_dir.parent().is_some() {
                                    self.current_dir.pop();
                                    self.selected = 0;
                                    self.update_items();
                                }
                            }
                            ActiveTab::PlaylistNavigation => {
                                if self.viewing_playlist.is_some() {
                                    self.viewing_playlist = None;
                                    self.playlist_track_selected = 0;
                                }
                            }
                            _ => {}
                        }
                    }
                },
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    if let ActiveTab::FolderNavigation = self.active_tab {
                        if !self.items.is_empty() {
                            let selected_item = &self.items[self.selected];
                            if !selected_item.starts_with("[DIR]") {
                                let track_path = self.current_dir.join(selected_item);
                                if let Some(index) = self.marked_tracks.iter().position(|p| p == &track_path) {
                                    self.marked_tracks.remove(index);
                                } else {
                                    self.marked_tracks.push(track_path);
                                }
                            }
                        }
                    }
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    match self.active_tab {
                        ActiveTab::FolderNavigation => {
                            let tracks: Vec<PathBuf> = std::fs::read_dir(&self.current_dir)
                                .unwrap_or_else(|_| std::fs::read_dir(".").unwrap())
                                .filter_map(|res| res.ok())
                                .map(|entry| entry.path())
                                .filter(|p| p.is_file() && Self::is_audio_file(p))
                                .collect();

                            if !tracks.is_empty() {
                                self.current_folder = self.current_dir.file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|s| s.to_string());
                                self.is_playing = true;
                                self.is_paused = false;
                                self.is_shuffle_mode = false;
                                self.player.send(PlayerCommand::PlayAlbum(tracks));
                            }
                        }
                        ActiveTab::PlaylistNavigation => {
                            if let Some(playlist_index) = self.viewing_playlist {
                                let playlist = &self.playlists[playlist_index];
                                let valid_tracks: Vec<PathBuf> = playlist.tracks
                                    .iter()
                                    .filter(|track| track.exists() && Self::is_audio_file(track))
                                    .cloned()
                                    .collect();

                                if !valid_tracks.is_empty() {
                                    self.current_folder = Some(format!("Playlist: {}", playlist.name));
                                    self.is_playing = true;
                                    self.is_paused = false;
                                    self.is_shuffle_mode = false;
                                    self.player.send(PlayerCommand::PlayAlbum(valid_tracks));
                                }
                            } else if !self.playlists.is_empty() {
                                let playlist = &self.playlists[self.playlist_selected];
                                let valid_tracks: Vec<PathBuf> = playlist.tracks
                                    .iter()
                                    .filter(|track| track.exists() && Self::is_audio_file(track))
                                    .cloned()
                                    .collect();

                                if !valid_tracks.is_empty() {
                                    self.current_folder = Some(format!("Playlist: {}", playlist.name));
                                    self.is_playing = true;
                                    self.is_paused = false;
                                    self.is_shuffle_mode = false;
                                    self.player.send(PlayerCommand::PlayAlbum(valid_tracks));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    match self.active_tab {
                        ActiveTab::FolderNavigation => {
                            let mut tracks: Vec<PathBuf> = WalkDir::new(&self.current_dir)
                                .into_iter()
                                .filter_map(|e| e.ok())
                                .map(|e| e.into_path())
                                .filter(|p| p.is_file() && Self::is_audio_file(p))
                                .collect();

                            if !tracks.is_empty() {
                                let mut rng = rand::rng();
                                tracks.shuffle(&mut rng);
                                self.current_folder = self.current_dir.file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|s| s.to_string());
                                self.is_playing = true;
                                self.is_paused = false;
                                self.is_shuffle_mode = true;
                                self.player.send(PlayerCommand::PlayShuffle(tracks));
                            }
                        }
                        ActiveTab::PlaylistNavigation => {
                            let playlist_to_shuffle = if let Some(playlist_index) = self.viewing_playlist {
                                Some(&self.playlists[playlist_index])
                            } else if !self.playlists.is_empty() {
                                Some(&self.playlists[self.playlist_selected])
                            } else {
                                None
                            };

                            if let Some(playlist) = playlist_to_shuffle {
                                let mut valid_tracks: Vec<PathBuf> = playlist.tracks
                                    .iter()
                                    .filter(|track| track.exists() && Self::is_audio_file(track))
                                    .cloned()
                                    .collect();

                                if !valid_tracks.is_empty() {
                                    let mut rng = rand::rng();
                                    valid_tracks.shuffle(&mut rng);

                                    self.current_folder = Some(format!("Playlist: {} (shuffle)", playlist.name));
                                    self.is_playing = true;
                                    self.is_paused = false;
                                    self.is_shuffle_mode = true;
                                    self.player.send(PlayerCommand::PlayShuffle(valid_tracks));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.player.send(PlayerCommand::SkipNext);
                }
                KeyCode::Char(' ') => {
                    if self.is_playing {
                        self.is_paused = !self.is_paused;
                        self.player.send(PlayerCommand::TogglePause);
                    }
                }
                KeyCode::Char('z') | KeyCode::Char('Z') => {
                    self.player.send(PlayerCommand::VolumeDown);
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    self.player.send(PlayerCommand::VolumeUp);
                }
                KeyCode::Backspace => {
                    self.current_folder = None;
                    self.is_playing = false;
                    self.is_paused = false;
                    self.is_shuffle_mode = false;
                    self.player.send(PlayerCommand::Stop);
                }
                _ => {}
            }
        }
        Ok(false)
    }

        fn ui(&self, f: &mut Frame) {
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(f.size());

        self.draw_header(f, main_chunks[0]);
        self.draw_main_content(f, main_chunks[1]);
        self.draw_status(f, main_chunks[2]);

        if self.is_creating_playlist {
            self.draw_playlist_creation_popup(f);
        }

        if self.is_deleting_playlist {
            self.draw_delete_confirmation_popup(f);
        }
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let header_text = "  ↑/↓ nav | Enter sel | Space pause | P play album | S shuffle | B search | z/x vol | Esc back | Q quit ";
        let header = Block::default()
            .title("rusted-player")
            .title_style(Style::default().add_modifier(Modifier::BOLD))
            .borders(Borders::ALL);
        let header_paragraph = Paragraph::new(header_text)
            .block(header)
            .wrap(Wrap { trim: true });
        f.render_widget(header_paragraph, area);
    }

    fn draw_main_content(&self, f: &mut Frame, area: Rect) {
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0)
            ])
            .split(area);

        self.draw_tabs(f, content_chunks[0]);

        match self.active_tab {
            ActiveTab::FolderNavigation => self.draw_folder_navigation(f, content_chunks[1]),
            ActiveTab::PlaylistNavigation => self.draw_playlist_navigation(f, content_chunks[1]),
            ActiveTab::Stats => self.draw_stats(f, content_chunks[1]),
        }
    }

    fn draw_tabs(&self, f: &mut Frame, area: Rect) {
        let titles = vec!["1 folder-navigation", "2 playlist-navigation", "3 stats"];
        let tabs = ratatui::widgets::Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("tabs"))
            .select(self.active_tab as usize)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        f.render_widget(tabs, area);
    }

    fn draw_folder_navigation(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if self.is_searching {
                vec![Constraint::Length(3), Constraint::Min(0)]
            } else {
                vec![Constraint::Min(0)]
            })
            .split(area);

        if self.is_searching {
            let search_input = Paragraph::new(self.search_query.as_str())
                .block(Block::default().borders(Borders::ALL).title("Search (Esc to cancel)"));
            f.render_widget(search_input, chunks[0]);
        }

        let content_area = if self.is_searching { chunks[1] } else { area };

        let folder_title = format!("[{}]", self.current_dir.to_string_lossy());

        let list_items: Vec<ListItem> = self
            .filtered_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let track_path = self.current_dir.join(item.strip_prefix("[DIR] ").unwrap_or(item));
                let style = if self.marked_tracks.contains(&track_path) {
                    Style::default().fg(Color::Green)
                } else if i == self.selected {
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Yellow)
                } else if item.starts_with("[DIR]") {
                    Style::default().fg(Color::Blue)
                } else {
                    Style::default()
                };
                ListItem::new(item.as_str()).style(style)
            })
            .collect();

        let list = List::new(list_items)
            .block(Block::default()
                .title(folder_title)
                .title_style(Style::default().add_modifier(Modifier::BOLD))
                .borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray)
                    .fg(Color::White)
            )
            .highlight_symbol("> ");

        let mut list_state = ListState::default();
        if !self.filtered_items.is_empty() {
            list_state.select(Some(self.selected));
        }

        f.render_stateful_widget(list, content_area, &mut list_state);
    }

    fn draw_playlist_navigation(&self, f: &mut Frame, area: Rect) {
        if let Some(playlist_index) = self.viewing_playlist {
            let playlist = &self.playlists[playlist_index];
            let title = format!("Playlist: {} ({} tracks)", playlist.name, playlist.tracks.len());

            let list_items: Vec<ListItem> = playlist.tracks
                .iter()
                .enumerate()
                .map(|(i, track)| {
                    let track_name = track.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown");

                    let style = if i == self.playlist_track_selected {
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(Color::Yellow)
                    } else if track.exists() {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::Red)
                    };

                    let display_text = if track.exists() {
                        format!("{}", track_name)
                    } else {
                        format!("{} [MISSING]", track_name)
                    };

                    ListItem::new(display_text).style(style)
                })
                .collect();

            let list = List::new(list_items)
                .block(Block::default()
                    .title(title)
                    .title_style(Style::default().add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL))
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                )
                .highlight_symbol("> ");

            let mut list_state = ListState::default();
            if !playlist.tracks.is_empty() {
                list_state.select(Some(self.playlist_track_selected));
            }

            f.render_stateful_widget(list, area, &mut list_state);
        } else {
            let title = format!("Playlists ({} total)", self.playlists.len());

            if self.playlists.is_empty() {
                let placeholder = Paragraph::new("No playlists found\nCreate playlists in folder navigation using 'L' to mark tracks and 'C' to name them. If you want to delete one, press 'D'.")
                    .block(Block::default().borders(Borders::ALL).title(title))
                    .wrap(Wrap { trim: true });
                f.render_widget(placeholder, area);
            } else {
                let list_items: Vec<ListItem> = self.playlists
                    .iter()
                    .enumerate()
                    .map(|(i, playlist)| {
                        let valid_tracks = playlist.tracks.iter()
                            .filter(|track| track.exists())
                            .count();

                        let display_text = format!("{} ({}/{})",
                                                   playlist.name,
                                                   valid_tracks,
                                                   playlist.tracks.len()
                        );

                        let style = if i == self.playlist_selected {
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        };

                        ListItem::new(display_text).style(style)
                    })
                    .collect();

                let list = List::new(list_items)
                    .block(Block::default()
                        .title(title)
                        .title_style(Style::default().add_modifier(Modifier::BOLD))
                        .borders(Borders::ALL))
                    .highlight_style(
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .bg(Color::DarkGray)
                            .fg(Color::White)
                    )
                    .highlight_symbol("> ");

                let mut list_state = ListState::default();
                list_state.select(Some(self.playlist_selected));

                f.render_stateful_widget(list, area, &mut list_state);
            }
        }
    }

    fn draw_stats(&self, f: &mut Frame, area: Rect) {
        let stats = self.playlist_service.get_stats();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let table_data: Vec<Row> = vec![
            Row::new(vec!["Total Tracks".to_string(), stats.total_tracks.to_string()]),
            Row::new(vec!["Total Genres".to_string(), stats.total_genres.to_string()]),
            Row::new(vec!["Total Albums".to_string(), stats.total_albums.to_string()]),
            Row::new(vec!["Total Duration".to_string(), stats.format_duration()]),
        ];

        let table = Table::new(table_data, &[Constraint::Percentage(50), Constraint::Percentage(50)])
            .block(Block::default().title("Stats").borders(Borders::ALL));

        f.render_widget(table, chunks[0]);

        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(bottom_chunks[0]);

        let mut top_genres_data: Vec<(String, usize)> = self.playlist_service.get_genres()
            .iter()
            .map(|genre| {
                (genre.clone(), self.playlist_service.get_playlist_by_genre(genre).len())
            })
            .collect();
        top_genres_data.sort_by(|a, b| b.1.cmp(&a.1));
        top_genres_data.truncate(5);

        let top_genres_list = List::new(
            top_genres_data.iter().enumerate().map(|(i, (genre, count))| {
                ListItem::new(format!("{}. {} ({})", i + 1, genre, count))
            }).collect::<Vec<ListItem>>(),
        )
            .block(Block::default().title("Top-Genres").borders(Borders::ALL));

        f.render_widget(top_genres_list, left_chunks[0]);

        let top_artists_data = self.playlist_service.get_top_artists();
        let top_artists_list = List::new(
            top_artists_data.iter().enumerate().map(|(i, (artist, count))| {
                ListItem::new(format!("{}. {} ({})", i + 1, artist, count))
            }).collect::<Vec<ListItem>>(),
        )
            .block(Block::default().title("Top-Artists").borders(Borders::ALL));

        f.render_widget(top_artists_list, left_chunks[1]);

        let decades = self.playlist_service.get_tracks_by_decade();
        let mut decade_data: Vec<(&str, u64)> = decades
            .iter()
            .map(|(decade, count)| (decade.as_str(), *count))
            .collect();

        decade_data.sort_by_key(|k| k.0);

        let decade_barchart = BarChart::default()
            .block(Block::default().title("Decades").borders(Borders::ALL))
            .data(decade_data.as_slice())
            .bar_width(9)
            .bar_style(Style::default().fg(Color::Green))
            .value_style(Style::default().fg(Color::Black).bg(Color::Green));

        f.render_widget(decade_barchart, bottom_chunks[1]);
    }

    fn draw_playlist_creation_popup(&self, f: &mut Frame) {
        let popup_area = Self::centered_rect(60, 40, f.size());
        f.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(popup_area);

        let title = Block::default().title("Add to Playlist").borders(Borders::ALL);
        f.render_widget(title, popup_area);

        let mut items = self.playlists.iter().map(|p| ListItem::new(p.name.as_str())).collect::<Vec<_>>();
        items.push(ListItem::new("Create new playlist..."));

        let list = List::new(items)
            .block(Block::default().borders(Borders::NONE))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray));

        let mut list_state = ListState::default();
        list_state.select(Some(self.playlist_creation_selected));

        f.render_stateful_widget(list, chunks[1], &mut list_state);

        if self.playlist_creation_selected == self.playlists.len() {
            let input = Paragraph::new(self.playlist_name_input.as_str())
                .block(Block::default().borders(Borders::ALL).title("New Playlist Name"));
            f.render_widget(input, chunks[2]);
        }
    }

    fn draw_delete_confirmation_popup(&self, f: &mut Frame) {
        let popup_area = Self::centered_rect(40, 20, f.size());
        f.render_widget(Clear, popup_area);

        let text = if let Some(index) = self.playlist_to_delete {
            format!("Are you sure you want to delete playlist '{}'? (y/n)", self.playlists[index].name)
        } else {
            String::new()
        };

        let popup = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Delete Playlist"));
        f.render_widget(popup, popup_area);
    }

    fn draw_status(&self, f: &mut Frame, area: Rect) {
        let status_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(90), Constraint::Percentage(10)])
            .split(area);

        let status_text = if self.is_paused {
            if self.is_shuffle_mode {
                format!("⏸ Paused: {} in shuffle mode", self.current_folder.as_deref().unwrap_or(""))
            } else {
                format!("⏸ Paused: {}", self.current_folder.as_deref().unwrap_or(""))
            }
        } else if self.is_playing {
            if self.is_shuffle_mode {
                format!("♪ Playing: {} in shuffle mode", self.current_folder.as_deref().unwrap_or(""))
            } else {
                format!("♪ Playing: {}", self.current_folder.as_deref().unwrap_or(""))
            }
        } else {
            "No album selected".to_string()
        };

        let status_paragraph = Paragraph::new(status_text)
            .block(Block::default()
                .title("status")
                .title_style(Style::default().add_modifier(Modifier::BOLD))
                .borders(Borders::ALL));
        f.render_widget(status_paragraph, status_chunks[0]);

        let volume_text = format!("Vol: {:.0}/20", self.volume * 10.0);
        let volume_paragraph = Paragraph::new(volume_text)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(volume_paragraph, status_chunks[1]);
    }

    fn is_audio_file(path: &PathBuf) -> bool {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            matches!(ext.to_lowercase().as_str(), "mp3" | "flac" | "ogg" | "wav")
        } else {
            false
        }
    }

    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

}