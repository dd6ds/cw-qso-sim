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
    /// CWT contest exchange: 4-digit member number or state/country for non-members
    pub cwt_ex:     String,
}

impl SimExchange {
    pub fn generate<R: Rng>(rng: &mut R, style: QsoStyle) -> Self {
        // For DARC CW contest always pick a German station so DOK is never "NM"
        let st = if style == QsoStyle::DarcCwContest {
            random_dl_station(rng)
        } else {
            random_station(rng)
        };
        Self {
            sim_call:   st.call.to_string(),
            sim_name:   st.name.to_string(),
            sim_qth:    st.qth.to_string(),
            // German (DL) stations are always DARC members — draw a random DOK
            // from the full 1192-code pool so each QSO feels realistic.
            // All other countries keep their fixed dok field ("NM" for non-members).
            dok:        if st.country == "DL" {
                            random_dok(rng).to_string()
                        } else {
                            st.dok.to_string()
                        },
            rst_to_me:  random_rst(rng).to_string(),
            rig:        random_rig(rng).to_string(),
            ant:        random_ant(rng).to_string(),
            pwr:        random_pwr(rng).to_string(),
            // Sim is already mid-contest — pick a plausible serial (1-250)
            sim_serial: rng.gen_range(1u32..=250),
            // CWT exchange: members get a random realistic number (1000-9999).
            // Non-members keep their country/state code (e.g. "G", "CA", "DL").
            cwt_ex:     if st.cwt_ex.parse::<u32>().is_ok() {
                            rng.gen_range(1000u32..=9999).to_string()
                        } else {
                            st.cwt_ex.to_string()
                        },
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
    /// `cwt_name` / `cwt_nr` are the user's own CWT exchange fields (name + member nr or state/country).
    pub fn build(mycall: &str, ex: &SimExchange, style: QsoStyle, my_rst: &str, my_serial: u32,
                 cwt_name: &str, cwt_nr: &str, my_dok: &str) -> Self {
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

        // ── CWT Contest: Name + member number (or state/country) ──────────────
        // Exchange pattern (sim calls CQ, user answers):
        //   SIM → CQ CQ CWT <sim> K
        //   USR → <sim> DE <my> KN
        //   SIM → <my> <sim_name> <sim_cwt_ex> K        (e.g. "DD6DS HANS 1812 K")
        //   USR → TU <cwt_name> <cwt_nr> K              (e.g. "TU DENNIS 2345 K")
        //   SIM → TU <sim>                               ← final ack, QSO done
        if style == QsoStyle::CwtContest {
            let cq         = format!("CQ CQ CWT {sc} K");
            let answer     = format!("{mycall} DE {sc} {sc} K");
            let report     = format!("{mycall} {sn} {} K", ex.cwt_ex);
            // SIM's final ack — sent after the user's exchange. QSO is done after this.
            let ack_report = format!("TU {sc}");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),   // not reached for CWT
                // Hint shown to the user: what they should send back
                contest_ex: format!("TU {cwt_name} {cwt_nr}"),
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
                // contest_ex is the hint showing what the USER should send back
                contest_ex: format!("{sc} UR RST 599 DOK {my_dok} {my_dok} <AR>"),
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
