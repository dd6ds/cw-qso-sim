// src/qso/exchanges.rs  —  Build human-like QSO exchange sentences
use rand::Rng;
use super::callsigns::*;
use crate::config::QsoStyle;

pub struct SimExchange {
    pub sim_call:   String,
    pub sim_name:   String,
    pub sim_qth:    String,
    pub dok:        String,
    pub rst_to_me:  String,
    pub rig:        String,
    pub ant:        String,
    pub pwr:        String,
    /// QSO serial number for the sim station (used in MWC / contest exchanges)
    pub sim_serial: u32,
}

impl SimExchange {
    pub fn generate<R: Rng>(rng: &mut R) -> Self {
        let st = random_station(rng);
        Self {
            sim_call:   st.call.to_string(),
            sim_name:   st.name.to_string(),
            sim_qth:    st.qth.to_string(),
            dok:        st.dok.to_string(),
            rst_to_me:  random_rst(rng).to_string(),
            rig:        random_rig(rng).to_string(),
            ant:        random_ant(rng).to_string(),
            pwr:        random_pwr(rng).to_string(),
            // Sim is already mid-contest — pick a plausible serial (1-250)
            sim_serial: rng.gen_range(1u32..=250),
        }
    }
}

/// Build all the messages the SIM sends during a QSO
pub struct QsoScript {
    pub cq:         String,  // CQ CQ DE <sim> <sim> K
    pub answer:     String,  // <my> DE <sim> <sim> K  (answer to my CQ)
    pub report:     String,  // <my> DE <sim> UR RST … NAME … QTH … <AR>
    pub ack_report: String,  // TU <my> RST … NAME … QTH … RIG … ANT … PWR … HW? <AR>
    pub chat:       Vec<String>, // rag-chew follow-ups
    pub sign_off:   String,  // TU FR QSO 73 GL DE <sim> <SK>
    pub contest_ex: String,  // RST NR for contest
}

impl QsoScript {
    /// `my_serial` is the user's running QSO count (001, 002, …).
    /// It appears in the MWC contest_ex hint and is used as the number
    /// the user should send back to the sim station.
    pub fn build(mycall: &str, ex: &SimExchange, style: QsoStyle, my_rst: &str, my_serial: u32) -> Self {
        let sc  = &ex.sim_call;
        let sn  = &ex.sim_name;
        let sq  = &ex.sim_qth;
        let sr  = &ex.rst_to_me;
        let dok = &ex.dok;
        let rig = &ex.rig;
        let ant = &ex.ant;
        let pwr = &ex.pwr;

        // ── MWC Contest: RST + running serial number ───────────────────────────
        // Exchange pattern (sim calls CQ, user answers):
        //   SIM → CQ CQ TEST <sim> K
        //   USR → <sim> DE <my> <my> K
        //   SIM → <my> UR RST 5NN 5NN <sim_serial> K
        //   USR → <sim> UR RST 5NN 5NN <my_serial> K
        //   SIM → <my> TU 73 <SK>          ← combined ack + sign-off
        //   USR → <sim> TU 73 <SK>         ← user echoes, sim waits for this
        if style == QsoStyle::MwcContest {
            let sim_ser = ex.sim_serial;
            let cq         = format!("CQ CQ TEST {sc} K");
            let answer     = format!("{mycall} DE {sc} {sc} K");
            let report     = format!("{mycall} UR RST {sr} {sr} {sim_ser:03} K");
            // Combined ack + sign-off — sent after the user's report.
            // The sim goes directly to WaitFor73 after this, no separate sign-off.
            let ack_report = format!("{mycall} TU 73 <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),   // not reached for MWC
                // Hint shown to the user: what they should send back
                contest_ex: format!("{sc} UR RST 599 599 {my_serial:03} K"),
            };
        }

        // ── DARC CW Contest: only RST + DOK exchanged ─────────────────────────
        if style == QsoStyle::DarcCwContest {
            let cq = format!("CQ TEST DE {sc} {sc} K");
            let answer = format!("{mycall} DE {sc} {sc} K");
            // sim sends its RST + DOK (NM if not a DARC member / not DL)
            let report = format!(
                "{mycall} DE {sc} RST {sr} {sr} DOK {dok} {dok} <AR>"
            );
            // sim acks with TU + our RST (sim doesn't know our DOK, just confirms)
            let ack_report = format!("TU {mycall} RST {my_rst} {my_rst} <AR>");
            let sign_off   = format!("TU 73 DE {sc} <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off,
                contest_ex: format!("{mycall} DE {sc} {sr} {dok} <AR>"),
            };
        }

        // ── All other styles ───────────────────────────────────────────────────
        let cq = format!("CQ CQ DE {sc} {sc} K");
        let answer = format!("{mycall} DE {sc} {sc} K");

        let report = format!(
            "{mycall} DE {sc} GE OM UR {sr} {sr} NAME {sn} {sn} QTH {sq} {sq} HW? <AR>"
        );

        let ack_report = format!(
            "TU {mycall} UR {my_rst} {my_rst} \
             NAME {sn} QTH {sq} RIG {rig} ANT {ant} PWR {pwr} \
             HW? <AR>"
        );

        let chat = vec![
            format!("WX HR FINE TEMP WARM HW UR WX? <AR>"),
            format!("RIG HR {rig} ANT {ant} PWR {pwr} HW UR RIG? <AR>"),
            format!("BEEN LIC MANY YRS NW ENJOY CW VY MUCH HW? <AR>"),
            format!("HR WE HAVE NICE QSB TODAY HW? <AR>"),
        ];

        let sign_off = format!("OK {sn} TU FB QSO 73 ES GL DE {sc} <SK>");
        let contest_ex = format!("{mycall} DE {sc} 599 001 001 <AR>");

        Self { cq, answer, report, ack_report, chat, sign_off, contest_ex }
    }
}
