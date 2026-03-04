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

// ── Translated UI label set ────────────────────────────────────────────────────
struct Labels {
    my:               &'static str,
    you:              &'static str,
    sim_tx_title:     &'static str,
    your_input_title: &'static str,
    decoded:          &'static str,
    typing:           &'static str,
    current:          &'static str,
    status:           &'static str,
    decode_off:       &'static str,
    footer_demo:      &'static str,
    footer_text:      &'static str,
    footer_keyer:     &'static str,
}

impl Labels {
    fn new(lang: &str) -> Self {
        match lang {
            "de" => Self {
                my:               "MEIN",
                you:              "DU",
                sim_tx_title:     " SIM SENDET ",
                your_input_title: " DEINE EINGABE ",
                decoded:          "DEKODIERT:",
                typing:           "EINGABE:  ",
                current:          "TASTE:    ",
                status:           "STATUS:   ",
                decode_off:       "[ DEKODIERUNG AUS ]",
                footer_demo:  " DEMO-MODUS — SIM spielt das gesamte QSO automatisch   ESC = Beenden",
                footer_text:  " Rufzeichen/Austausch tippen   Leerzeichen = Wort   Enter = Over senden (K)   Esc = Beenden",
                footer_keyer: " Hardware-Keyer aktiv   Q = Beenden   Esc = Beenden",
            },
            "fr" => Self {
                my:               "MOI",
                you:              "VOUS",
                sim_tx_title:     " SIM TX ",
                your_input_title: " VOTRE SAISIE ",
                decoded:          "DÉCODÉ:  ",
                typing:           "FRAPPE:  ",
                current:          "ACTUEL:  ",
                status:           "STATUT:  ",
                decode_off:       "[ DÉCODAGE DÉSACTIVÉ ]",
                footer_demo:  " MODE DÉMO — SIM joue le QSO complet automatiquement   ESC = quitter",
                footer_text:  " Saisir l'indicatif/échange   Espace = mot   Entrée = fin d'over (K)   Esc = quitter",
                footer_keyer: " Manipulateur actif   Q = quitter   Esc = quitter",
            },
            "it" => Self {
                my:               "MIO",
                you:              "TU",
                sim_tx_title:     " SIM TX ",
                your_input_title: " TUA IMMISSIONE ",
                decoded:          "DECODIF: ",
                typing:           "DIGITA:  ",
                current:          "ATTUALE: ",
                status:           "STATO:   ",
                decode_off:       "[ DECODIFICA DISATTIVATA ]",
                footer_demo:  " MODALITÀ DEMO — SIM riproduce il QSO automaticamente   ESC = uscita",
                footer_text:  " Digita nominativo/scambio   Spazio = parola   Invio = fine over (K)   Esc = uscita",
                footer_keyer: " Manipolatore attivo   Q = uscita   Esc = uscita",
            },
            _ => Self {  // English (default)
                my:               "MY",
                you:              "YOU",
                sim_tx_title:     " SIM TX ",
                your_input_title: " YOUR INPUT ",
                decoded:          "DECODED: ",
                typing:           "TYPING:  ",
                current:          "CURRENT: ",
                status:           "STATUS:  ",
                decode_off:       "[ DECODE OFF ]",
                footer_demo:  " DEMO MODE — SIM plays the full QSO automatically   ESC = exit",
                footer_text:  " Type callsign/exchange   Space = word   Enter = send over (K)   Esc = quit",
                footer_keyer: " Hardware keyer active   Q = quit   Esc = quit",
            },
        }
    }
}

pub struct Tui {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    labels:   Labels,
}

impl Tui {
    pub fn new(lang: &str) -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen)?;
        let backend  = CrosstermBackend::new(out);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal, labels: Labels::new(lang) })
    }

    pub fn cleanup(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
    }

    pub fn draw(&mut self, s: &AppState) -> Result<()> {
        let lb = &self.labels;
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
                " CW QSO Simulator  |  {}: {}  ←→  SIM: {}  |  SIM: {}WPM  {}: {}WPM  {}Hz",
                lb.my, s.mycall, s.sim_call, s.sim_wpm, lb.you, s.user_wpm, s.tone_hz
            ))
            .style(Style::default().fg(Color::Black).bg(Color::Cyan)
                   .add_modifier(Modifier::BOLD));
            f.render_widget(header, chunks[0]);

            // ── SIM TX ────────────────────────────────────────────────────
            let sim_text: Vec<Line> = if s.no_decode {
                vec![Line::from(Span::styled(
                    lb.decode_off,
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
                ))]
            } else {
                s.sim_log.iter()
                    .map(|l| Line::from(Span::styled(
                        l.clone(),
                        Style::default().fg(Color::Green),
                    )))
                    .collect()
            };
            let sim_block = Paragraph::new(sim_text)
                .block(Block::default()
                    .title(format!("{}({}) ", lb.sim_tx_title, s.sim_call))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)))
                .wrap(Wrap { trim: true });
            f.render_widget(sim_block, chunks[1]);

            // ── User decoded ──────────────────────────────────────────────
            let user_lines: Vec<Line> = if s.no_decode {
                vec![
                    Line::from(Span::styled(
                        lb.decode_off,
                        Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(format!("{} ", lb.status), Style::default().fg(Color::DarkGray)),
                        Span::styled(s.status.clone(), Style::default().fg(Color::Magenta)),
                    ]),
                ]
            } else {
                vec![
                    Line::from(vec![
                        Span::styled(format!("{} ", lb.decoded), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::styled(s.user_decoded.clone(), Style::default().fg(Color::White)),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            format!("{} ", if s.text_mode { lb.typing } else { lb.current }),
                            Style::default().fg(Color::DarkGray)
                        ),
                        Span::styled(s.current_code.clone(), Style::default().fg(Color::Cyan)),
                    ]),
                    Line::from(vec![
                        Span::styled(format!("{} ", lb.status), Style::default().fg(Color::DarkGray)),
                        Span::styled(s.status.clone(), Style::default().fg(Color::Magenta)),
                    ]),
                ]
            };
            let user_block = Paragraph::new(user_lines)
                .block(Block::default()
                    .title(lb.your_input_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)));
            f.render_widget(user_block, chunks[2]);

            // ── Footer ────────────────────────────────────────────────────
            let footer_text = if s.demo {
                lb.footer_demo
            } else if s.text_mode {
                lb.footer_text
            } else {
                lb.footer_keyer
            };
            let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray).bg(Color::Black));
            f.render_widget(footer, chunks[3]);
        })?;
        Ok(())
    }
}
