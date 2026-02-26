//! QSO state machine — simulates a realistic human CW partner.

use rand::Rng;
use super::callsigns::SimStation;

/// Phases of a standard ragchew QSO
#[derive(Debug, Clone, PartialEq)]
pub enum QsoPhase {
    Idle,
    CqSent,                 // sim sent CQ, waiting for user
    CqReceived,             // user called CQ, sim responds
    ExchangeFirst,          // sim sends first exchange (RST+name+QTH)
    WaitingUserExchange,    // waiting for user to send exchange
    ExchangeAck,            // sim acknowledges user exchange
    Ragchew(u8),            // ongoing conversation, step 0..N
    WaitingUserReply,       // sim sent a turn, waiting
    Closing,                // 73 exchange
    Done,
}

#[derive(Debug)]
pub struct QsoEngine {
    pub station:        SimStation,
    pub phase:          QsoPhase,
    pub last_sent:      String,     // last thing sim sent (for ? repeat)
    pub my_call:        String,
    pub his_rst:        u16,        // RST we will give him
}

impl QsoEngine {
    pub fn new(station: SimStation, my_call: &str, rng: &mut impl Rng) -> Self {
        let rst_options = [559u16, 569, 579, 589, 599];
        let his_rst = rst_options[rng.gen_range(0..rst_options.len())];
        Self {
            station,
            phase:     QsoPhase::Idle,
            last_sent: String::new(),
            my_call:   my_call.to_string(),
            his_rst,
        }
    }

    /// Sim speaks first (sends CQ)
    pub fn sim_starts_msg(&mut self) -> String {
        let msg = format!("CQ CQ CQ DE {} {} K",
            self.station.callsign, self.station.callsign);
        self.last_sent = msg.clone();
        self.phase = QsoPhase::CqSent;
        msg
    }

    /// User speaks first (they send CQ), sim responds
    pub fn respond_to_user_cq(&mut self, user_input: &str) -> String {
        // Detect if user is calling CQ
        let msg = format!("{} DE {} {} RST {} {} ES {} QTH {} BK",
            self.my_call,
            self.station.callsign,
            self.station.callsign,
            self.station.rst_rx,
            self.station.name,
            self.station.qth,
            self.station.qth,
        );
        self.last_sent = msg.clone();
        self.phase = QsoPhase::WaitingUserExchange;
        msg
    }

    /// User sends their exchange after sim's CQ
    pub fn respond_to_user_exchange(&mut self, _input: &str) -> String {
        let msg = format!(
            "TU {} DE {} RST {} {} NAME {} QTH {} RIG {} ANT {} PWR {}W BK",
            self.my_call,
            self.station.callsign,
            self.station.rst_rx,
            self.station.rst_rx,
            self.station.name,
            self.station.qth,
            self.station.rig,
            self.station.ant,
            self.station.power_w,
        );
        self.last_sent = msg.clone();
        self.phase = QsoPhase::Ragchew(0);
        msg
    }

    /// Continue the rag-chew conversation
    pub fn ragchew_reply(&mut self, _input: &str, rng: &mut impl Rng, step: u8) -> String {
        let msg = match step {
            0 => format!(
                "FB {} HW CPY QSL RST {} {} HW? BK",
                self.my_call, self.his_rst, self.his_rst
            ),
            1 => format!(
                "TNX {} ES FB SIG PSE QSL UR {} {} HW? BK",
                self.station.name, self.his_rst, self.his_rst
            ),
            2 => {
                let wx = ["WX FB SUNSINE", "WX CLOUDY", "WX RAIN", "WX SNOW"];
                let w  = wx[rng.gen_range(0..wx.len())];
                format!("{} {} ES TNX QSO 73 DE {} SK", self.my_call, w, self.station.callsign)
            }
            _ => format!("73 TU ES GUD DX DE {} SK", self.station.callsign),
        };
        self.last_sent = msg.clone();
        if step >= 2 {
            self.phase = QsoPhase::Closing;
        } else {
            self.phase = QsoPhase::Ragchew(step + 1);
        }
        msg
    }

    /// Handle user input, advance state machine
    pub fn process_user_input(&mut self, input: &str, rng: &mut impl Rng) -> Option<String> {
        let upper = input.trim().to_uppercase();

        // "?" = repeat last transmission
        if upper == "?" || upper == "AGN" || upper == "PSE AGN" {
            return Some(format!("AGN: {}", self.last_sent));
        }

        let response = match &self.phase.clone() {
            QsoPhase::CqSent => {
                // User answered our CQ
                self.respond_to_user_exchange(&upper)
            }
            QsoPhase::WaitingUserExchange => {
                self.respond_to_user_exchange(&upper)
            }
            QsoPhase::Ragchew(step) => {
                let s = *step;
                self.ragchew_reply(&upper, rng, s)
            }
            QsoPhase::WaitingUserReply => {
                let step = 0u8;
                self.ragchew_reply(&upper, rng, step)
            }
            QsoPhase::Closing | QsoPhase::Done => {
                self.phase = QsoPhase::Done;
                format!("73 DE {} SK", self.station.callsign)
            }
            _ => return None,
        };

        Some(response)
    }

    pub fn is_done(&self) -> bool {
        self.phase == QsoPhase::Done
    }
}
