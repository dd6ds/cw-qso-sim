// src/qso/exchanges.rs  —  Build human-like QSO exchange sentences
use rand::Rng;
use super::callsigns::{
    random_station, random_dl_station, random_wwa_callsign,
    random_dok, random_rst, random_rig, random_ant, random_pwr,
    random_pota_ref, random_sota_ref, random_tota_ref, random_cota_ref,
};
use crate::config::QsoStyle;

pub struct SimExchange {
    pub sim_call:       String,
    pub sim_name:       String,
    pub sim_qth:        String,
    pub dok:            String,
    pub rst_to_me:      String,
    pub rig:            String,
    pub ant:            String,
    pub pwr:            String,
    /// QSO serial number for the sim station (used in MWC / contest exchanges)
    pub sim_serial:     u32,
    /// CWT contest exchange: 4-digit member number or state/country for non-members
    pub cwt_ex:         String,
    /// SST SPC: US/VE/VK state or province; DXCC prefix for other countries
    pub spc:            String,
    /// POTA park ref (K-XXXX), SOTA summit ref (W1/WR-001), TOTA tower ref (US-XXXX)
    pub activator_ref:  String,
}

impl SimExchange {
    pub fn generate<R: Rng>(rng: &mut R, style: QsoStyle) -> Self {
        // For DARC CW contest always pick a German station so DOK is never "NM"
        let st = if style == QsoStyle::DarcCwContest {
            random_dl_station(rng)
        } else {
            random_station(rng)
        };
        // For WWA contest use an official WWA special station callsign
        let sim_call = if style == QsoStyle::WwaContest {
            random_wwa_callsign(rng).to_string()
        } else {
            st.call.to_string()
        };
        let activator_ref = match style {
            QsoStyle::Pota => random_pota_ref(rng, st.country),
            QsoStyle::Sota => random_sota_ref(rng, st.country),
            QsoStyle::Tota => random_tota_ref(rng, st.country),
            QsoStyle::Cota => random_cota_ref(rng, st.country),
            _              => String::new(),
        };

        Self {
            sim_call,
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
            spc:        st.spc.to_string(),
            activator_ref,
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
        let spc = &ex.spc;

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
            let ack_report = format!("{mycall} RST {my_rst} {my_rst} <AR>");
            let sign_off   = format!("TU 73 DE {sc} <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off,
                // contest_ex is the hint showing what the USER should send back
                contest_ex: format!("{sc} UR RST 599 DOK {my_dok} {my_dok} <AR>"),
            };
        }

        // ── WPX Contest: RST + serial number ──────────────────────────────────
        // Exchange pattern (sim calls CQ, user answers):
        //   SIM → CQ WPX TEST <sim> <sim> K
        //   USR → <my> K
        //   SIM → <my> <rst> <sim_serial> K
        //   USR → <rst> <my_serial> K           ← just RST + serial, no callsign
        //   SIM → TU QSL 73                     ← combined ack + sign-off, QSO done
        if style == QsoStyle::WpxContest {
            let sim_ser = ex.sim_serial;
            let cq         = format!("CQ WPX TEST {sc} {sc} K");
            let answer     = format!("{mycall} DE {sc} {sc} K");
            let report     = format!("{mycall} {sr} {sim_ser:03} K");
            // Combined ack + sign-off — QSO is done immediately after this.
            let ack_report = format!("TU QSL 73");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),   // not reached for WPX
                // Hint shown to the user: just RST + their serial, no callsign
                contest_ex: format!("599 {my_serial:03} K"),
            };
        }

        // ── WWA Contest: RST + serial number (sent twice) + BK ────────────────
        // Exchange pattern (sim calls CQ, user answers):
        //   SIM → CQ WWA <sim> <sim> K
        //   USR → <my> <my> K
        //   SIM → <my> DE <sim> <rst> <sim_serial> <sim_serial> BK
        //   USR → <sim> DE <my> 599 <my_serial> <my_serial> BK
        //   SIM → R TU 73 <SK>          ← combined ack + sign-off
        //   USR → 73                    ← user echoes, sim waits for this
        if style == QsoStyle::WwaContest {
            let sim_ser = ex.sim_serial;
            let cq         = format!("CQ WWA {sc} {sc} K");
            let answer     = format!("{mycall} DE {sc} {sc} K");
            let report     = format!("{mycall} DE {sc} {sr} {sim_ser:03} {sim_ser:03} BK");
            // Combined ack + sign-off — sent after the user's report.
            // The sim goes directly to WaitFor73 after this, no separate sign-off.
            let ack_report = format!("R TU 73 <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),   // not reached for WWA
                // Hint shown to the user: what they should send back
                contest_ex: format!("{sc} DE {mycall} 599 {my_serial:03} {my_serial:03} BK"),
            };
        }

        // ── QTT Award: Quality True Telegraphist — full rag-chew with RSN ─────
        // Rules from https://www.no5nn.org/award/
        //   • RSN (Readability-Strength-Note) replaces RST
        //   • Minimum exchange: RSN + Name + QTH + PWR + ANT
        //   • CQ must end with K (never bare callsign or <AR> alone)
        //   • Sign-off uses "77" ("Long Live CW") instead of "73"
        //   • Both sides must send 77 before QSO is complete
        //
        // Exchange pattern:
        //   SIM → CQ CQ DE <sim> <sim> K
        //   USR → <sim> DE <my> K
        //   SIM → <my> DE <sim> GE OM UR RSN <rsn> NAME <name> QTH <qth> PWR <pwr> ANT <ant> HW? KN
        //   USR → TU RSN 599 NAME OP QTH HOME PWR 100W ANT DIPOLE HW? KN
        //   SIM → (ack + optional chat turns)
        //   SIM → OK <name> TU FB QSO 77 ES GL DE <sim> <SK>
        //   USR → 77                     ← sim waits for this
        if style == QsoStyle::QttAward {
            let cq = format!("CQ CQ DE {sc} {sc} K");
            let answer = format!("{mycall} DE {sc} {sc} K");

            // Full exchange: RSN + name + QTH + PWR + ANT
            let report = format!(
                "{mycall} DE {sc} GE OM UR RSN {sr} {sr} \
                 NAME {sn} {sn} QTH {sq} {sq} \
                 PWR {pwr} ANT {ant} HW? KN"
            );

            // SIM acks user's exchange and sends its own RSN + rig info
            let ack_report = format!(
                "TU {mycall} UR RSN {my_rst} {my_rst} \
                 NAME {sn} QTH {sq} RIG {rig} ANT {ant} PWR {pwr} \
                 HW? <AR>"
            );

            let chat = vec![
                format!("WX HR FINE TEMP WARM HW UR WX? <AR>"),
                format!("UR SIG VY FB HR NICE QSO ES GD QTT PROCEDURE HW? <AR>"),
                format!("BEEN OPS MANY YRS NW ENJOY QTT STDS VY MUCH HW? <AR>"),
            ];

            // Sign-off uses "77" (Long Live CW) instead of "73"
            let sign_off = format!("OK {sn} TU FB QSO 77 ES GL DE {sc} <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat,
                sign_off,
                // Hint: what the user should send back (RSN + name + QTH + PWR + ANT)
                contest_ex: format!(
                    "{sc} DE {mycall} TU RSN 599 NAME OP QTH HOME PWR 100W ANT DIPOLE HW? KN"
                ),
            };
        }

        // ── SST (Slow Speed CW Contest): Name + SPC, no RST ───────────────────
        // Exchange pattern (sim calls CQ, user answers):
        //   SIM → CQ SST <sim> K
        //   USR → <my> K                      ← just callsign
        //   SIM → <my> <sim_name> <sim_spc>   ← name + state/country, no RST!
        //   USR → GE <sim_name> <name> <spc>  ← greeting + their name + my name + my SPC
        //   SIM → GL <name> TU <sim> K        ← ack using user's name, QSO done
        if style == QsoStyle::SstContest {
            let cq         = format!("CQ SST {sc} K");
            let answer     = format!("{mycall} DE {sc} K");
            let report     = format!("{mycall} {sn} {spc}");
            // SIM acks using the user's configured name (cwt_name) — combined sign-off, QSO done
            let ack_report = format!("GL {cwt_name} TU {sc} K");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),   // not reached for SST
                // Hint: greeting + SIM name + user name + user SPC
                contest_ex: format!("GE {sn} {cwt_name} {cwt_nr}"),
            };
        }

        // ── CQ DX: International DX QSO — RST + Name + QTH exchange ──────────
        // Exchange pattern (sim calls CQ DX, user answers):
        //   SIM → CQ DX CQ DX CQ DX DE <sim> <sim> <sim> K
        //   USR → <sim> DE <my> <my> 599 599 TU K     ← callsign + RST
        //   SIM → <my> DE <sim> <rst> <rst> TU NAME <name> <name> QTH <qth> <qth> BT HW? BK
        //   USR → <sim> DE <my> 559 559 NAME OP QTH HOME BT QSL TU 73 SK
        //   SIM → <my> DE <sim> 73 <SK>               ← final ack
        if style == QsoStyle::CqDx {
            let cq     = format!("CQ DX CQ DX DE {sc} {sc} K");
            let answer = format!("{mycall} DE {sc} {sc} K");
            let report = format!(
                "{mycall} DE {sc} {sr} {sr} TU NAME {sn} {sn} QTH {sq} {sq} BT HW? BK"
            );
            // Final ack — QSO done immediately after this
            let ack_report = format!("{mycall} DE {sc} 73 <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),   // not reached for CqDx
                // Hint: what the user should send back
                contest_ex: format!(
                    "{sc} DE {mycall} {my_rst} {my_rst} TU NAME OP QTH HOME BT QSL TU 73 SK"
                ),
            };
        }

        // ── POTA (Parks on the Air): RST + park reference ─────────────────────
        // Exchange pattern (sim activates a park, calls CQ POTA):
        //   SIM → CQ POTA CQ POTA DE <sim> <sim> K
        //   USR → <sim> DE <my> K
        //   SIM → <my> <rst> <park_ref> K
        //   USR → TU 73 K           ← acknowledge, QSO done
        if style == QsoStyle::Pota {
            let park = &ex.activator_ref;
            let cq         = format!("CQ POTA CQ POTA DE {sc} {sc} K");
            let answer     = format!("{mycall} DE {sc} {sc} K");
            let report     = format!("{mycall} {sr} {park} K");
            let ack_report = format!("73 DE {sc} <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),
                contest_ex: format!("TU 73 K"),
            };
        }

        // ── SOTA (Summits on the Air): RST + summit reference ─────────────────
        // SIM operates portable (/P) from a summit.
        // Exchange pattern:
        //   SIM → CQ SOTA DE <sim>/P <sim>/P K
        //   USR → <sim>/P DE <my> K
        //   SIM → <my> <rst> <summit_ref> K
        //   USR → TU 73 K           ← acknowledge, QSO done
        if style == QsoStyle::Sota {
            let summit = &ex.activator_ref;
            let sp         = format!("{sc}/P");  // portable suffix
            let cq         = format!("CQ SOTA DE {sp} {sp} K");
            let answer     = format!("{mycall} DE {sp} {sp} K");
            let report     = format!("{mycall} {sr} {summit} K");
            let ack_report = format!("73 DE {sp} <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),
                contest_ex: format!("TU 73 K"),
            };
        }

        // ── TOTA (Towers on the Air): RST + tower reference ───────────────────
        // Exchange pattern (sim activates a tower, calls CQ TOTA):
        //   SIM → CQ TOTA CQ TOTA DE <sim> <sim> K
        //   USR → <sim> DE <my> K
        //   SIM → <my> <rst> <tower_ref> K
        //   USR → TU 73 K           ← acknowledge, QSO done
        if style == QsoStyle::Tota {
            let tower = &ex.activator_ref;
            let cq         = format!("CQ TOTA CQ TOTA DE {sc} {sc} K");
            let answer     = format!("{mycall} DE {sc} {sc} K");
            let report     = format!("{mycall} {sr} {tower} K");
            let ack_report = format!("73 DE {sc} <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),
                contest_ex: format!("TU 73 K"),
            };
        }

        // ── COTA (Castles on the Air): RST + castle reference ─────────────────
        // Exchange pattern (sim activates a castle, calls CQ COTA):
        //   SIM → CQ COTA CQ COTA DE <sim> <sim> K
        //   USR → <sim> DE <my> K
        //   SIM → <my> <rst> <castle_ref> K
        //   USR → TU 73 K           ← acknowledge, QSO done
        if style == QsoStyle::Cota {
            let castle = &ex.activator_ref;
            let cq         = format!("CQ COTA CQ COTA DE {sc} {sc} K");
            let answer     = format!("{mycall} DE {sc} {sc} K");
            let report     = format!("{mycall} {sr} {castle} K");
            let ack_report = format!("73 DE {sc} <SK>");

            return Self {
                cq, answer, report, ack_report,
                chat:       vec![],
                sign_off:   String::new(),
                contest_ex: format!("TU 73 K"),
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
