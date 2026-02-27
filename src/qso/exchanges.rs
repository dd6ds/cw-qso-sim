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
}

impl SimExchange {
    pub fn generate<R: Rng>(rng: &mut R) -> Self {
        let st = random_station(rng);
        Self {
            sim_call:  st.call.to_string(),
            sim_name:  st.name.to_string(),
            sim_qth:   st.qth.to_string(),
            dok:       st.dok.to_string(),
            rst_to_me: random_rst(rng).to_string(),
            rig:       random_rig(rng).to_string(),
            ant:       random_ant(rng).to_string(),
            pwr:       random_pwr(rng).to_string(),
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
    pub fn build(mycall: &str, ex: &SimExchange, style: QsoStyle, my_rst: &str) -> Self {
        let sc  = &ex.sim_call;
        let sn  = &ex.sim_name;
        let sq  = &ex.sim_qth;
        let sr  = &ex.rst_to_me;
        let dok = &ex.dok;
        let rig = &ex.rig;
        let ant = &ex.ant;
        let pwr = &ex.pwr;

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
