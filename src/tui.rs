use std::io::stdout;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color as RColor, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
    Terminal,
};

use crate::color::Color;
use crate::device::{HardwareEffect, LenovoKeyboard};
use crate::effects::AnimationEffect;
use crate::profile::{default_profiles, load_profiles, save_profile, LightingProfile, ProfileMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPanel {
    Zones,
    Sliders,
    Presets,
    Effects,
    Profiles,
}

pub struct AppState {
    pub keyboard: LenovoKeyboard,
    pub zones: [Color; 4],
    pub active_zone: usize, // 0..3 or 4 for ALL
    pub brightness: u8,     // 0, 1, 2
    pub speed: u8,          // 1..4
    pub active_effect: Option<HardwareEffect>,
    pub active_animation: Option<AnimationEffect>,
    pub focused_panel: FocusedPanel,
    pub slider_idx: usize, // 0 = Red, 1 = Green, 2 = Blue
    pub preset_idx: usize,
    pub profile_idx: usize,
    pub profiles: Vec<LightingProfile>,
    pub status_message: String,
    pub should_quit: bool,
    pub save_dialog_open: bool,
    pub save_input: String,
    pub last_frame_time: Instant,
    pub start_time: Instant,
    pub sysinfo: Option<sysinfo::System>,
}

const PRESETS: &[(&str, Color)] = &[
    ("Legion Cyan", Color::LEGION_CYAN),
    ("Ice Blue", Color::ICE_BLUE),
    ("Deep Blue", Color::BLUE),
    ("Emerald Green", Color::GREEN),
    ("Acid Green", Color::ACID_GREEN),
    ("Amber Gold", Color::GOLD),
    ("Warm Orange", Color::ORANGE),
    ("Crimson Red", Color::RED),
    ("Neon Pink", Color::PINK),
    ("Neon Purple", Color::PURPLE),
    ("Magenta", Color::MAGENTA),
    ("Warm White", Color::new(255, 230, 180)),
    ("Pure White", Color::WHITE),
    ("Black / Off", Color::BLACK),
];

impl AppState {
    pub fn new(keyboard: LenovoKeyboard) -> Self {
        let profiles = load_profiles().unwrap_or_else(|_| default_profiles());
        Self {
            keyboard,
            zones: [Color::LEGION_CYAN; 4],
            active_zone: 4, // default to All Zones
            brightness: 2,
            speed: 2,
            active_effect: Some(HardwareEffect::Static),
            active_animation: None,
            focused_panel: FocusedPanel::Sliders,
            slider_idx: 0,
            preset_idx: 0,
            profile_idx: 0,
            profiles,
            status_message: "Ready. Use Tab or Arrow keys to navigate.".to_string(),
            should_quit: false,
            save_dialog_open: false,
            save_input: String::new(),
            last_frame_time: Instant::now(),
            start_time: Instant::now(),
            sysinfo: Some(sysinfo::System::new_all()),
        }
    }

    pub fn current_selected_color(&self) -> Color {
        if self.active_zone < 4 {
            self.zones[self.active_zone]
        } else {
            self.zones[0]
        }
    }

    pub fn update_active_color(&mut self, col: Color) {
        if self.active_zone < 4 {
            self.zones[self.active_zone] = col;
        } else {
            self.zones = [col; 4];
        }
        self.apply_current_lighting();
    }

    pub fn apply_current_lighting(&mut self) {
        if let Some(effect) = self.active_effect {
            let _ = self
                .keyboard
                .set_hardware_effect(effect, self.speed, self.brightness, &self.zones);
        } else if self.active_animation.is_none() {
            let _ = self.keyboard.set_static(&self.zones, self.brightness);
        }
    }

    pub fn tick_animation(&mut self) {
        if let Some(ref anim) = self.active_animation {
            let elapsed = self.start_time.elapsed().as_secs_f32();
            let speed_factor = match self.speed {
                1 => 0.5,
                2 => 1.0,
                3 => 1.8,
                _ => 2.8,
            };
            let frame = anim.get_frame(elapsed, speed_factor, &mut self.sysinfo);
            let _ = self.keyboard.set_static(&frame, self.brightness);
            self.zones = frame;
        }
    }
}

pub fn run_tui(keyboard: LenovoKeyboard) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(keyboard);
    state.apply_current_lighting();

    let tick_rate = Duration::from_millis(33); // ~30 FPS for smooth animations
    let mut last_tick = Instant::now();

    while !state.should_quit {
        terminal.draw(|f| ui(f, &state))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_input(&mut state, key.code, key.modifiers);
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            state.tick_animation();
            last_tick = Instant::now();
        }
    }

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn handle_input(state: &mut AppState, key: KeyCode, modifiers: KeyModifiers) {
    if state.save_dialog_open {
        match key {
            KeyCode::Enter => {
                let name = state.save_input.trim().to_string();
                if !name.is_empty() {
                    let prof = LightingProfile::new_static(
                        name.clone(),
                        "User saved profile from TUI",
                        state.zones,
                        state.brightness,
                    );
                    let _ = save_profile(prof);
                    state.profiles = load_profiles().unwrap_or_else(|_| default_profiles());
                    state.status_message = format!("Saved profile '{}'!", name);
                }
                state.save_dialog_open = false;
                state.save_input.clear();
            }
            KeyCode::Esc => {
                state.save_dialog_open = false;
                state.save_input.clear();
            }
            KeyCode::Backspace => {
                state.save_input.pop();
            }
            KeyCode::Char(c) => {
                state.save_input.push(c);
            }
            _ => {}
        }
        return;
    }

    match key {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.should_quit = true;
        }
        KeyCode::Tab => {
            state.focused_panel = match state.focused_panel {
                FocusedPanel::Zones => FocusedPanel::Sliders,
                FocusedPanel::Sliders => FocusedPanel::Presets,
                FocusedPanel::Presets => FocusedPanel::Effects,
                FocusedPanel::Effects => FocusedPanel::Profiles,
                FocusedPanel::Profiles => FocusedPanel::Zones,
            };
        }
        KeyCode::BackTab => {
            state.focused_panel = match state.focused_panel {
                FocusedPanel::Zones => FocusedPanel::Profiles,
                FocusedPanel::Sliders => FocusedPanel::Zones,
                FocusedPanel::Presets => FocusedPanel::Sliders,
                FocusedPanel::Effects => FocusedPanel::Presets,
                FocusedPanel::Profiles => FocusedPanel::Effects,
            };
        }
        KeyCode::Char('1') => {
            state.active_zone = 0;
            state.focused_panel = FocusedPanel::Sliders;
        }
        KeyCode::Char('2') => {
            state.active_zone = 1;
            state.focused_panel = FocusedPanel::Sliders;
        }
        KeyCode::Char('3') => {
            state.active_zone = 2;
            state.focused_panel = FocusedPanel::Sliders;
        }
        KeyCode::Char('4') => {
            state.active_zone = 3;
            state.focused_panel = FocusedPanel::Sliders;
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            state.active_zone = 4; // All zones
            state.focused_panel = FocusedPanel::Sliders;
        }
        KeyCode::Char('b') | KeyCode::Char('B') => {
            state.brightness = (state.brightness + 1) % 3;
            state.apply_current_lighting();
            state.status_message = format!("Brightness set to {}", state.brightness);
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            state.save_dialog_open = true;
            state.save_input = format!("Profile-{}", state.profiles.len() + 1);
        }
        _ => match state.focused_panel {
            FocusedPanel::Zones => match key {
                KeyCode::Left | KeyCode::Char('h') => {
                    if state.active_zone > 0 {
                        state.active_zone -= 1;
                    } else {
                        state.active_zone = 4;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if state.active_zone < 4 {
                        state.active_zone += 1;
                    } else {
                        state.active_zone = 0;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    state.focused_panel = FocusedPanel::Sliders;
                }
                _ => {}
            },

            FocusedPanel::Sliders => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.slider_idx > 0 {
                        state.slider_idx -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if state.slider_idx < 2 {
                        state.slider_idx += 1;
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    let step = if modifiers.contains(KeyModifiers::SHIFT) { 25 } else { 5 };
                    let mut col = state.current_selected_color();
                    match state.slider_idx {
                        0 => col.r = col.r.saturating_sub(step),
                        1 => col.g = col.g.saturating_sub(step),
                        2 => col.b = col.b.saturating_sub(step),
                        _ => {}
                    }
                    state.active_effect = Some(HardwareEffect::Static);
                    state.active_animation = None;
                    state.update_active_color(col);
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    let step = if modifiers.contains(KeyModifiers::SHIFT) { 25 } else { 5 };
                    let mut col = state.current_selected_color();
                    match state.slider_idx {
                        0 => col.r = col.r.saturating_add(step),
                        1 => col.g = col.g.saturating_add(step),
                        2 => col.b = col.b.saturating_add(step),
                        _ => {}
                    }
                    state.active_effect = Some(HardwareEffect::Static);
                    state.active_animation = None;
                    state.update_active_color(col);
                }
                _ => {}
            },

            FocusedPanel::Presets => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.preset_idx > 0 {
                        state.preset_idx -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if state.preset_idx + 1 < PRESETS.len() {
                        state.preset_idx += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let (name, col) = PRESETS[state.preset_idx];
                    state.active_effect = Some(HardwareEffect::Static);
                    state.active_animation = None;
                    state.update_active_color(col);
                    state.status_message = format!("Applied preset '{}'", name);
                }
                _ => {}
            },

            FocusedPanel::Effects => match key {
                KeyCode::Char('w') => {
                    state.active_effect = Some(HardwareEffect::Wave);
                    state.active_animation = None;
                    state.apply_current_lighting();
                    state.status_message = "Applied Hardware Wave effect".to_string();
                }
                KeyCode::Char('p') => {
                    state.active_effect = Some(HardwareEffect::Breath);
                    state.active_animation = None;
                    state.apply_current_lighting();
                    state.status_message = "Applied Hardware Breath effect".to_string();
                }
                KeyCode::Char('u') => {
                    state.active_effect = Some(HardwareEffect::Hue);
                    state.active_animation = None;
                    state.apply_current_lighting();
                    state.status_message = "Applied Hardware Spectrum Cycle (Hue)".to_string();
                }
                KeyCode::Char('t') => {
                    state.active_effect = Some(HardwareEffect::Static);
                    state.active_animation = None;
                    state.apply_current_lighting();
                    state.status_message = "Switched to Static Mode".to_string();
                }
                KeyCode::Char('r') => {
                    state.active_effect = None;
                    state.active_animation = Some(AnimationEffect::RainbowWave);
                    state.status_message = "Running Software Rainbow Wave".to_string();
                }
                KeyCode::Char('c') => {
                    state.active_effect = None;
                    state.active_animation = Some(AnimationEffect::CpuMonitor);
                    state.status_message = "Running Live CPU Load Monitor".to_string();
                }
                KeyCode::Char('f') => {
                    state.active_effect = None;
                    state.active_animation = Some(AnimationEffect::Fire);
                    state.status_message = "Running Campfire Animation".to_string();
                }
                KeyCode::Char('m') => {
                    state.active_effect = None;
                    state.active_animation = Some(AnimationEffect::Matrix);
                    state.status_message = "Running Matrix Rain Animation".to_string();
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    state.speed = (state.speed % 4) + 1;
                    state.apply_current_lighting();
                    state.status_message = format!("Speed set to {}", state.speed);
                }
                KeyCode::Char('-') => {
                    state.speed = if state.speed > 1 { state.speed - 1 } else { 4 };
                    state.apply_current_lighting();
                    state.status_message = format!("Speed set to {}", state.speed);
                }
                _ => {}
            },

            FocusedPanel::Profiles => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.profile_idx > 0 {
                        state.profile_idx -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if state.profile_idx + 1 < state.profiles.len() {
                        state.profile_idx += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(prof) = state.profiles.get(state.profile_idx).cloned() {
                        state.zones = prof.zones;
                        state.brightness = prof.brightness;
                        state.speed = prof.speed;
                        match prof.mode {
                            ProfileMode::Static => {
                                state.active_effect = Some(HardwareEffect::Static);
                                state.active_animation = None;
                            }
                            ProfileMode::Breath => {
                                state.active_effect = Some(HardwareEffect::Breath);
                                state.active_animation = None;
                            }
                            ProfileMode::Wave => {
                                state.active_effect = Some(HardwareEffect::Wave);
                                state.active_animation = None;
                            }
                            ProfileMode::Hue => {
                                state.active_effect = Some(HardwareEffect::Hue);
                                state.active_animation = None;
                            }
                            ProfileMode::CpuMonitor => {
                                state.active_effect = None;
                                state.active_animation = Some(AnimationEffect::CpuMonitor);
                            }
                            ProfileMode::RainbowWave => {
                                state.active_effect = None;
                                state.active_animation = Some(AnimationEffect::RainbowWave);
                            }
                            _ => {
                                state.active_effect = Some(HardwareEffect::Static);
                                state.active_animation = None;
                            }
                        }
                        state.apply_current_lighting();
                        state.status_message = format!("Applied profile '{}'", prof.name);
                    }
                }
                _ => {}
            },
        },
    }
}

fn to_ratatui_color(c: Color) -> RColor {
    RColor::Rgb(c.r, c.g, c.b)
}

fn ui(f: &mut ratatui::Frame, state: &AppState) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(7),  // Keyboard Zone Visualizer
            Constraint::Min(12),    // Controls / Sliders / Presets / Profiles
            Constraint::Length(3),  // Help / Keybindings Footer
        ])
        .split(size);

    // Header
    let info = state.keyboard.device_info();
    let header_text = vec![
        Line::from(vec![
            Span::styled(" Lenovo Vantage RGB Controller ", Style::default().fg(RColor::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" [Rust Native] ", Style::default().fg(RColor::Yellow)),
            Span::raw(format!("• Device: {} ({:04x}:{:04x}) on {:?}", info.name, info.vendor_id, info.product_id, state.keyboard.path())),
        ]),
    ];
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(header, chunks[0]);

    // Keyboard Zone Visualizer
    render_keyboard_zones(f, chunks[1], state);

    // Main Control Area: Split Horizontally (Sliders & Adjuster | Presets & Effects | Profiles)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Zone Sliders & Color Adjuster
            Constraint::Percentage(32), // Presets & Effects
            Constraint::Percentage(28), // Saved Profiles
        ])
        .split(chunks[2]);

    render_sliders(f, main_chunks[0], state);
    render_presets_and_effects(f, main_chunks[1], state);
    render_profiles(f, main_chunks[2], state);

    // Footer / Help Bar
    let status_style = Style::default().fg(RColor::Green);
    let footer_text = vec![
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(RColor::DarkGray)),
            Span::styled(&state.status_message, status_style),
        ]),
        Line::from(vec![
            Span::styled("[Tab]", Style::default().fg(RColor::Yellow)), Span::raw(" Switch  "),
            Span::styled("[1-4/A]", Style::default().fg(RColor::Yellow)), Span::raw(" Zone  "),
            Span::styled("[←/→]", Style::default().fg(RColor::Yellow)), Span::raw(" Adjust  "),
            Span::styled("[B]", Style::default().fg(RColor::Yellow)), Span::raw(" Brightness  "),
            Span::styled("[S]", Style::default().fg(RColor::Yellow)), Span::raw(" Save Profile  "),
            Span::styled("[Q]", Style::default().fg(RColor::Red)), Span::raw(" Quit"),
        ]),
    ];
    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(footer, chunks[3]);

    // Save Dialog Popup
    if state.save_dialog_open {
        let popup_area = centered_rect(50, 20, size);
        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Save Current Profile ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(RColor::Yellow));

        let input_text = vec![
            Line::from("Enter profile name and press Enter:"),
            Line::from(""),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(RColor::Cyan)),
                Span::styled(&state.save_input, Style::default().fg(RColor::White).add_modifier(Modifier::BOLD)),
                Span::styled("_", Style::default().fg(RColor::Yellow)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Enter] Save    [Esc] Cancel", Style::default().fg(RColor::DarkGray)),
            ]),
        ];
        let popup = Paragraph::new(input_text).block(block);
        f.render_widget(popup, popup_area);
    }
}

fn render_keyboard_zones(f: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(" Keyboard Backlight Preview (4 Zones) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if state.focused_panel == FocusedPanel::Zones {
            Style::default().fg(RColor::Cyan)
        } else {
            Style::default().fg(RColor::Gray)
        });

    let inner = block.inner(area);
    f.render_widget(block, area);

    let zone_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(inner);

    let zone_names = [
        "Zone 1: Left / Esc",
        "Zone 2: Mid-Left",
        "Zone 3: Mid-Right",
        "Zone 4: Numpad / Nav",
    ];

    for i in 0..4 {
        let col = state.zones[i];
        let is_selected = state.active_zone == i || state.active_zone == 4;

        let bg = to_ratatui_color(col);
        // Contrast calculation for text
        let brightness_lum = (col.r as f32 * 0.299 + col.g as f32 * 0.587 + col.b as f32 * 0.114) as u8;
        let fg = if brightness_lum > 140 { RColor::Black } else { RColor::White };

        let border_col = if is_selected { RColor::Yellow } else { RColor::DarkGray };

        let zone_block = Block::default()
            .title(zone_names[i])
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_col));

        let zone_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("  {}  ", col.to_hex()), Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled(
                    if is_selected { " [ACTIVE] " } else { "          " },
                    Style::default().fg(if is_selected { RColor::Yellow } else { RColor::Reset }).add_modifier(Modifier::BOLD),
                ),
            ]),
        ];

        let p = Paragraph::new(zone_text)
            .block(zone_block)
            .alignment(Alignment::Center);
        f.render_widget(p, zone_chunks[i]);
    }
}

fn render_sliders(f: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let is_focused = state.focused_panel == FocusedPanel::Sliders;
    let border_style = if is_focused {
        Style::default().fg(RColor::Cyan)
    } else {
        Style::default().fg(RColor::Gray)
    };

    let title = if state.active_zone < 4 {
        format!(" Color Adjuster (Zone {}) ", state.active_zone + 1)
    } else {
        " Color Adjuster (ALL Zones) ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Zone selection tabs
            Constraint::Length(3), // Red
            Constraint::Length(3), // Green
            Constraint::Length(3), // Blue
            Constraint::Length(2), // Brightness info
        ])
        .split(inner);

    // Zone selection tabs
    let tabs = Tabs::new(vec!["Zone 1", "Zone 2", "Zone 3", "Zone 4", "ALL"])
        .select(state.active_zone)
        .highlight_style(Style::default().fg(RColor::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[0]);

    let col = state.current_selected_color();

    // Red Slider
    let r_gauge = Gauge::default()
        .block(Block::default().title(format!("Red: {} [0..255]", col.r)))
        .gauge_style(Style::default().fg(RColor::Red).bg(RColor::DarkGray))
        .ratio(col.r as f64 / 255.0)
        .label(format!("{}/255", col.r));
    f.render_widget(r_gauge, chunks[1]);

    // Green Slider
    let g_gauge = Gauge::default()
        .block(Block::default().title(format!("Green: {} [0..255]", col.g)))
        .gauge_style(Style::default().fg(RColor::Green).bg(RColor::DarkGray))
        .ratio(col.g as f64 / 255.0)
        .label(format!("{}/255", col.g));
    f.render_widget(g_gauge, chunks[2]);

    // Blue Slider
    let b_gauge = Gauge::default()
        .block(Block::default().title(format!("Blue: {} [0..255]", col.b)))
        .gauge_style(Style::default().fg(RColor::Blue).bg(RColor::DarkGray))
        .ratio(col.b as f64 / 255.0)
        .label(format!("{}/255", col.b));
    f.render_widget(b_gauge, chunks[3]);

    // Brightness indicator
    let b_label = match state.brightness {
        0 => "Off (0)",
        1 => "Low (1)",
        _ => "High (2)",
    };
    let b_para = Paragraph::new(format!("Hardware Brightness: {} (Press 'b' to cycle)", b_label))
        .style(Style::default().fg(RColor::Yellow));
    f.render_widget(b_para, chunks[4]);
}

fn render_presets_and_effects(f: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(48), // Quick Presets
            Constraint::Percentage(52), // Hardware Effects & Animations
        ])
        .split(area);

    // Quick Presets
    let is_preset_focused = state.focused_panel == FocusedPanel::Presets;
    let preset_items: Vec<ListItem> = PRESETS
        .iter()
        .enumerate()
        .map(|(i, (name, col))| {
            let is_sel = is_preset_focused && i == state.preset_idx;
            let bg = to_ratatui_color(*col);

            let prefix = if is_sel { "▶ " } else { "  " };
            let style = if is_sel {
                Style::default().fg(RColor::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(RColor::White)
            };

            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::styled("   ", Style::default().bg(bg)),
                Span::raw(" "),
                Span::styled(*name, style),
            ]))
        })
        .collect();

    let preset_list = List::new(preset_items).block(
        Block::default()
            .title(" Color Palette / Presets ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if is_preset_focused {
                Style::default().fg(RColor::Cyan)
            } else {
                Style::default().fg(RColor::Gray)
            }),
    );
    f.render_widget(preset_list, chunks[0]);

    // Hardware & Software Effects
    let is_effects_focused = state.focused_panel == FocusedPanel::Effects;
    let effects_text = vec![
        Line::from(vec![
            Span::styled("Hardware (EC Firmware):", Style::default().fg(RColor::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(" [t] Static   ", if state.active_effect == Some(HardwareEffect::Static) { Style::default().fg(RColor::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(RColor::White) }),
            Span::styled(" [w] Wave     ", if state.active_effect == Some(HardwareEffect::Wave) { Style::default().fg(RColor::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(RColor::White) }),
        ]),
        Line::from(vec![
            Span::styled(" [p] Breath   ", if state.active_effect == Some(HardwareEffect::Breath) { Style::default().fg(RColor::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(RColor::White) }),
            Span::styled(" [u] Hue Cycle", if state.active_effect == Some(HardwareEffect::Hue) { Style::default().fg(RColor::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(RColor::White) }),
        ]),
        Line::from(vec![
            Span::styled(format!(" Speed: {} [+/-] to change", state.speed), Style::default().fg(RColor::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Dynamic Software Animations:", Style::default().fg(RColor::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(" [r] Rainbow Wave  ", if state.active_animation == Some(AnimationEffect::RainbowWave) { Style::default().fg(RColor::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(RColor::White) }),
            Span::styled(" [c] CPU Monitor", if state.active_animation == Some(AnimationEffect::CpuMonitor) { Style::default().fg(RColor::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(RColor::White) }),
        ]),
        Line::from(vec![
            Span::styled(" [f] Campfire      ", if state.active_animation == Some(AnimationEffect::Fire) { Style::default().fg(RColor::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(RColor::White) }),
            Span::styled(" [m] Matrix Rain", if state.active_animation == Some(AnimationEffect::Matrix) { Style::default().fg(RColor::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(RColor::White) }),
        ]),
    ];

    let effects_p = Paragraph::new(effects_text)
        .block(
            Block::default()
                .title(" Effects & Animations ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if is_effects_focused {
                    Style::default().fg(RColor::Cyan)
                } else {
                    Style::default().fg(RColor::Gray)
                }),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(effects_p, chunks[1]);
}

fn render_profiles(f: &mut ratatui::Frame, area: Rect, state: &AppState) {
    let is_focused = state.focused_panel == FocusedPanel::Profiles;

    let items: Vec<ListItem> = state
        .profiles
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_sel = is_focused && i == state.profile_idx;
            let prefix = if is_sel { "▶ " } else { "  " };

            let name_style = if is_sel {
                Style::default().fg(RColor::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(RColor::White)
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(prefix),
                    Span::styled(&p.name, name_style),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(&p.description, Style::default().fg(RColor::DarkGray)),
                ]),
            ])
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Profiles [Enter to apply] ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if is_focused {
                Style::default().fg(RColor::Cyan)
            } else {
                Style::default().fg(RColor::Gray)
            }),
    );
    f.render_widget(list, area);
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
