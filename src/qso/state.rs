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
    WaitFor73,   // DarcCwContest / MwcContest / WwaContest: wait for user to send 73 after SIM sign-off
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
    pub my_dok:  String,
}

impl QsoEngine {
    /// `my_serial` is the user's running QSO counter (starts at 1).
    /// It is embedded in MWC contest exchange hints so the user knows
    /// which serial number to send back to the sim station.
    pub fn new(cfg: &AppConfig, my_serial: u32) -> Self {
        let mut rng = SmallRng::from_entropy();
        let ex      = SimExchange::generate(&mut rng, cfg.qso_style);
        let my_rst  = random_rst(&mut rng).to_string();
        let script  = QsoScript::build(&cfg.mycall, &ex, cfg.qso_style, &my_rst, my_serial,
                                       &cfg.cwt_name, &cfg.cwt_nr, &cfg.my_dok);

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
            my_dok: cfg.my_dok.clone(),
            script,
            exchange: ex,
            rng,
        }
    }

    /// Call every loop tick. Returns Some(event) when the SIM wants to do something.
    /// `user_input` is the trimmed, uppercased content of the last completed word
    /// from the user's paddle.  Empty string means nothing new was received.
    pub fn tick(&mut self, user_input: &str) -> Option<QsoEvent> {
        // Handle '?' at any phase — repeat last transmission.
        // Only a *standalone* '?' word (the IMI prosign ..--..) triggers repeat.
        // A '?' embedded inside another word (e.g. "HW?" in a QTT exchange)
        // must NOT match — otherwise demo mode would loop forever on QTT.
        if user_input.split_whitespace().any(|w| w == "?") && !self.last_tx.is_empty() {
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
                        // MWC: ack_report IS the sign-off ("TU 73 <SK>"), so
                        // skip the separate SignOff phase and wait for the user's 73.
                        // WWA: same pattern — ack_report is "R TU 73 <SK>", then wait for user 73.
                        QsoStyle::MwcContest | QsoStyle::WwaContest => Phase::WaitFor73,
                        // CWT / WPX / SST / CqDx / POTA / SOTA / TOTA: ack_report is the
                        // final transmission — QSO done immediately.
                        QsoStyle::CwtContest | QsoStyle::WpxContest | QsoStyle::SstContest
                        | QsoStyle::CqDx
                        | QsoStyle::Pota | QsoStyle::Sota | QsoStyle::Tota | QsoStyle::Cota => Phase::Done,
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
                    // DARC CW Contest: sim sends 73 then waits for the user to
                    // reply with 73 before the QSO is considered done.
                    // QTT Award: sim sends 77 then waits for the user's 77 reply.
                    // (MWC / WWA never reach SignOff — they go WaitFor73 from SimAcksReport.)
                    self.phase = match self.style {
                        QsoStyle::DarcCwContest | QsoStyle::QttAward => Phase::WaitFor73,
                        _                       => Phase::Done,
                    };
                    Some(QsoEvent::SimTransmit(tx))
                } else { None }
            }

            Phase::WaitFor73 => {
                // Wait until the user sends "73" or "77" (QTT Award uses 77 = "Long Live CW")
                let up = user_input.to_uppercase();
                if up.contains("73") || up.contains("77") {
                    self.phase = Phase::Done;
                    None
                } else {
                    Some(QsoEvent::WaitingForUser)
                }
            }

            Phase::Done => Some(QsoEvent::QsoComplete),
        }
    }

    /// Returns true if `input` contains the user's own callsign (mycall).
    /// The SIM's callsign is NOT required — in real CW the SIM already
    /// knows its own call; accepting "DD6DS K" is just as valid as
    /// "SM5XY DE DD6DS K".  Requiring the SIM call caused silent
    /// failures whenever the user omitted or mistyped it.
    fn input_has_callsign(&self, input: &str) -> bool {
        let up = input.to_uppercase();
        !up.is_empty()
            && up.contains(&self.mycall.to_uppercase())
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

    /// Returns a plausible auto-response for the current phase.
    /// Used by `--demo` mode to drive the QSO without any keyer input.
    pub fn demo_response(&self) -> Option<String> {
        match &self.phase {
            // User answers the SIM's CQ
            // SST: just callsign; POTA/TOTA: callsign with DE; SOTA: /P suffix on sim call
            Phase::WaitForMyAnswer => Some(match self.style {
                QsoStyle::SstContest => format!("{} K", self.mycall),
                QsoStyle::Sota       => format!("{}/P DE {} K", self.exchange.sim_call, self.mycall),
                _                    => format!("{} DE {} K", self.exchange.sim_call, self.mycall),
            }),
            // User sends CQ (when who_starts = me)
            Phase::ISendCq => Some(format!("CQ CQ DE {} K", self.mycall)),
            // User sends their exchange
            Phase::WaitMyReport => {
                let sc = &self.exchange.sim_call;
                Some(match self.style {
                    QsoStyle::CwtContest => {
                        // contest_ex already holds "TU <name> <nr>"; append K
                        format!("{} K", self.script.contest_ex.trim())
                    }
                    QsoStyle::MwcContest => {
                        format!("{sc} UR RST 599 599 001 K")
                    }
                    QsoStyle::WwaContest => {
                        format!("{sc} DE {} 599 001 001 BK", self.mycall)
                    }
                    QsoStyle::WpxContest => {
                        // WPX: user sends only RST + their serial (no callsign)
                        format!("599 001 K")
                    }
                    QsoStyle::DarcCwContest => {
                        format!("{sc} UR RST 599 DOK {} {} AR", self.my_dok, self.my_dok)
                    }
                    QsoStyle::CqDx => {
                        format!("{sc} DE {} {my_rst} {my_rst} TU NAME OP QTH HOME BT QSL TU 73 SK",
                                self.mycall, my_rst = self.my_rst)
                    }
                    QsoStyle::Contest | QsoStyle::DxPileup => {
                        format!("{sc} UR RST 599 001 K")
                    }
                    QsoStyle::QttAward => {
                        // QTT: RSN (not RST) + name + QTH + PWR + ANT, end with KN
                        format!("{sc} DE {} TU RSN 599 NAME OP QTH HOME PWR 100W ANT DIPOLE HW? KN",
                                self.mycall)
                    }
                    QsoStyle::SstContest => {
                        // SST: greeting + SIM name + user name + user SPC (no RST!)
                        format!("GE {} OP MA", &self.exchange.sim_name)
                    }
                    QsoStyle::Pota | QsoStyle::Tota | QsoStyle::Cota => {
                        // Hunter just sends RST acknowledgment
                        format!("TU 599 K")
                    }
                    QsoStyle::Sota => {
                        // SOTA hunter acknowledges with RST
                        format!("TU 599 K")
                    }
                    _ => {
                        // Ragchew
                        format!("UR RST 579 NAME OP QTH HOME RIG IC7300 ANT DIPOLE K")
                    }
                })
            }
            // After sign-off: send 73, or 77 for QTT Award ("Long Live CW")
            Phase::WaitFor73 => Some(match self.style {
                QsoStyle::QttAward => "77 <SK>".to_string(),
                _                  => "73 K".to_string(),
            }),
            // Rag-chew conversation turn
            Phase::WaitChatReply => Some("FB OM TNX ES 73 K".to_string()),
            _ => None,
        }
    }
}
