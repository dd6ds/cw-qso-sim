// src/tui/inner.rs  —  ratatui layout
use anyhow::Result;
use crossterm::{execute, terminal::{self, EnterAlternateScreen, LeaveAlternateScreen}};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use crate::AppState;
use std::io::stdout;

pub struct Tui {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl Tui {
    pub fn new(_lang: &str) -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen)?;
        let backend  = CrosstermBackend::new(out);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn cleanup(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
    }

    pub fn draw(&mut self, s: &AppState) -> Result<()> {
        self.terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // header / status bar
                    Constraint::Min(6),     // SIM TX log
                    Constraint::Min(4),     // YOUR decoded text
                    Constraint::Length(3),  // footer hints
                ])
                .split(area);

            // ── Header ────────────────────────────────────────────────────
            let header = Paragraph::new(format!(
                " CW QSO Simulator  |  MY: {}  ←→  SIM: {}  |  SIM: {}WPM  YOU: {}WPM  {}Hz",
                s.mycall, s.sim_call, s.sim_wpm, s.user_wpm, s.tone_hz
            ))
            .style(Style::default().fg(Color::Black).bg(Color::Cyan)
                   .add_modifier(Modifier::BOLD));
            f.render_widget(header, chunks[0]);

            // ── SIM TX ────────────────────────────────────────────────────
            let sim_text: Vec<Line> = s.sim_log.iter()
                .map(|l| Line::from(Span::styled(
                    l.clone(),
                    Style::default().fg(Color::Green),
                )))
                .collect();
            let sim_block = Paragraph::new(sim_text)
                .block(Block::default()
                    .title(format!(" SIM TX  ({}) ", s.sim_call))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)))
                .wrap(Wrap { trim: true });
            f.render_widget(sim_block, chunks[1]);

            // ── User decoded ──────────────────────────────────────────────
            let user_lines: Vec<Line> = vec![
                Line::from(vec![
                    Span::styled("DECODED: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(s.user_decoded.clone(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled(
                        if s.text_mode { "TYPING:  " } else { "CURRENT: " },
                        Style::default().fg(Color::DarkGray)
                    ),
                    Span::styled(s.current_code.clone(), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::styled("STATUS:  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(s.status.clone(), Style::default().fg(Color::Magenta)),
                ]),
            ];
            let user_block = Paragraph::new(user_lines)
                .block(Block::default()
                    .title(" YOUR INPUT ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)));
            f.render_widget(user_block, chunks[2]);

            // ── Footer ────────────────────────────────────────────────────
            let footer_text = if s.demo {
                " DEMO MODE — SIM plays the full QSO automatically   ESC = exit"
            } else if s.text_mode {
                " Type callsign/exchange   Space = word   Enter = send over (K)   Esc = quit"
            } else {
                " Hardware keyer active   Q = quit   Esc = quit"
            };
            let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray).bg(Color::Black));
            f.render_widget(footer, chunks[3]);
        })?;
        Ok(())
    }
}
