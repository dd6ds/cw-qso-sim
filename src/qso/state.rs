// src/qso/state.rs  —  QSO state machine
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use std::time::{Duration, Instant};
use crate::config::{AppConfig, QsoStyle, WhoStarts};
use super::callsigns::random_rst;
use super::exchanges::{QsoScript, SimExchange};

/// Events produced by the engine for the UI / audio layer
#[derive(Debug, Clone)]
pub enum QsoEvent {
    SimTransmit(String),   // play this text as CW
    WaitingForUser,        // SIM is listening
    QsoComplete,           // QSO ended
    RepeatLast,            // user sent '?' → repeat last tx
}

/// QSO phases
#[derive(Debug, Clone, PartialEq)]
enum Phase {
    Init,
    SimSendsCq,
    WaitForMyAnswer,
    ISendCq,
    WaitForSimAnswer,
    SimSendsReport,
    WaitMyReport,
    SimAcksReport,
    Chat { turn: usize },
    WaitChatReply,
    SignOff,
    Done,
}

pub struct QsoEngine {
    phase:       Phase,
    script:      QsoScript,
    exchange:    SimExchange,
    last_tx:     String,
    my_rst:      String,
    next_tx_at:  Instant,
    min_delay:   Duration,
    max_delay:   Duration,
    rng:         SmallRng,
    pub mycall:  String,
    pub style:   QsoStyle,
    pub typo_rate: f64,
}

impl QsoEngine {
    pub fn new(cfg: &AppConfig) -> Self {
        let mut rng = SmallRng::from_entropy();
        let ex      = SimExchange::generate(&mut rng);
        let my_rst  = random_rst(&mut rng).to_string();
        let script  = QsoScript::build(&cfg.mycall, &ex, cfg.qso_style, &my_rst);

        let phase = match cfg.who_starts {
            WhoStarts::Sim => Phase::Init,
            WhoStarts::Me  => Phase::ISendCq,
        };

        Self {
            phase,
            my_rst,
            last_tx: String::new(),
            next_tx_at: Instant::now(),
            min_delay: Duration::from_millis(cfg.min_delay_ms),
            max_delay: Duration::from_millis(cfg.max_delay_ms),
            mycall: cfg.mycall.clone(),
            style:  cfg.qso_style,
            typo_rate: cfg.typo_rate,
            script,
            exchange: ex,
            rng,
        }
    }

    /// Call every loop tick. Returns Some(event) when the SIM wants to do something.
    /// `user_input` is the trimmed, uppercased content of the last completed word
    /// from the user's paddle.  Empty string means nothing new was received.
    pub fn tick(&mut self, user_input: &str) -> Option<QsoEvent> {
        // Handle '?' at any phase — repeat last transmission
        if user_input.contains('?') && !self.last_tx.is_empty() {
            return Some(QsoEvent::SimTransmit(self.last_tx.clone()));
        }

        let now = Instant::now();

        match &self.phase.clone() {
            Phase::Init => {
                self.schedule_delay();
                self.phase = Phase::SimSendsCq;
                None
            }

            Phase::SimSendsCq => {
                if now >= self.next_tx_at {
                    let tx = self.maybe_typo(&self.script.cq.clone());
                    self.last_tx = tx.clone();
                    self.phase   = Phase::WaitForMyAnswer;
                    Some(QsoEvent::SimTransmit(tx))
                } else { None }
            }

            Phase::WaitForMyAnswer => {
                // Only advance when the user has sent their own callsign.
                // A single stray character or partial word must not trigger this.
                if self.input_has_callsign(user_input) {
                    self.phase = Phase::SimSendsReport;
                    self.schedule_delay();
                    None
                } else {
                    Some(QsoEvent::WaitingForUser)
                }
            }

            Phase::ISendCq => {
                // Wait for the user to send CQ or a directed call
                if self.input_is_cq_or_call(user_input) {
                    self.schedule_delay();
                    self.phase = Phase::WaitForSimAnswer;
                    None
                } else {
                    Some(QsoEvent::WaitingForUser)
                }
            }

            Phase::WaitForSimAnswer => {
                if now >= self.next_tx_at {
                    let tx = self.maybe_typo(&self.script.answer.clone());
                    self.last_tx = tx.clone();
                    self.phase   = Phase::SimSendsReport;
                    self.schedule_delay();
                    Some(QsoEvent::SimTransmit(tx))
                } else { None }
            }

            Phase::SimSendsReport => {
                if now >= self.next_tx_at {
                    let tx = self.maybe_typo(&self.script.report.clone());
                    self.last_tx = tx.clone();
                    self.phase   = Phase::WaitMyReport;
                    Some(QsoEvent::SimTransmit(tx))
                } else { None }
            }

            Phase::WaitMyReport => {
                // Accept any meaningful exchange (at least 2 chars — RST, name, etc.)
                if user_input.len() >= 2 {
                    self.phase = Phase::SimAcksReport;
                    self.schedule_delay();
                    None
                } else {
                    Some(QsoEvent::WaitingForUser)
                }
            }

            Phase::SimAcksReport => {
                if now >= self.next_tx_at {
                    let tx = self.maybe_typo(&self.script.ack_report.clone());
                    self.last_tx = tx.clone();
                    let next_phase = match self.style {
                        QsoStyle::Contest | QsoStyle::DxPileup | QsoStyle::DarcCwContest => Phase::SignOff,
                        _ => Phase::Chat { turn: 0 },
                    };
                    self.phase = next_phase;
                    self.schedule_delay();
                    Some(QsoEvent::SimTransmit(tx))
                } else { None }
            }

            Phase::Chat { turn } => {
                let t = *turn;
                if now >= self.next_tx_at {
                    if t >= self.script.chat.len() {
                        self.phase = Phase::SignOff;
                        self.schedule_delay();
                        return None;
                    }
                    let tx = self.maybe_typo(&self.script.chat[t].clone());
                    self.last_tx = tx.clone();
                    self.phase   = Phase::WaitChatReply;
                    Some(QsoEvent::SimTransmit(tx))
                } else { None }
            }

            Phase::WaitChatReply => {
                // Accept any reply of at least 2 chars
                if user_input.len() >= 2 {
                    let next_turn = self.script.chat.iter()
                        .position(|m| m == &self.last_tx)
                        .map(|i| i + 1)
                        .unwrap_or(self.script.chat.len());
                    self.phase = Phase::Chat { turn: next_turn };
                    self.schedule_delay();
                    None
                } else {
                    Some(QsoEvent::WaitingForUser)
                }
            }

            Phase::SignOff => {
                if now >= self.next_tx_at {
                    let tx = self.maybe_typo(&self.script.sign_off.clone());
                    self.last_tx = tx.clone();
                    self.phase   = Phase::Done;
                    Some(QsoEvent::SimTransmit(tx))
                } else { None }
            }

            Phase::Done => Some(QsoEvent::QsoComplete),
        }
    }

    /// Returns true if `input` is a valid directed call to the SIM:
    /// must contain both the SIM's callsign (addressed to) and the
    /// user's own callsign (from).  E.g. "SM5XY DE DD6DS K".
    fn input_has_callsign(&self, input: &str) -> bool {
        let up = input.to_uppercase();
        !up.is_empty()
            && up.contains(&self.mycall.to_uppercase())
            && up.contains(&self.exchange.sim_call.to_uppercase())
    }

    /// Returns true if `input` looks like a CQ call from the user
    /// (contains "CQ") or a directed call that includes the user's callsign.
    fn input_is_cq_or_call(&self, input: &str) -> bool {
        let up = input.to_uppercase();
        !up.is_empty()
            && (up.contains("CQ") || up.contains(&self.mycall.to_uppercase()))
    }

    /// Simulate human typo: randomly insert <HH> + repeat
    fn maybe_typo(&mut self, text: &str) -> String {
        if self.rng.gen_bool(self.typo_rate) && text.len() > 4 {
            let words: Vec<&str> = text.split_whitespace().collect();
            let idx = self.rng.gen_range(0..words.len().max(1));
            let before: Vec<&str> = words[..=idx].to_vec();
            let after:  Vec<&str> = words.to_vec();
            // Insert HH after some word, then continue
            format!("{} <HH> {}", before.join(" "), after.join(" "))
        } else {
            text.to_string()
        }
    }

    fn schedule_delay(&mut self) {
        let ms = self.rng.gen_range(
            self.min_delay.as_millis() as u64 ..= self.max_delay.as_millis() as u64
        );
        self.next_tx_at = Instant::now() + Duration::from_millis(ms);
    }

    pub fn sim_callsign(&self) -> &str { &self.exchange.sim_call }
    pub fn is_done(&self) -> bool { self.phase == Phase::Done }
}
