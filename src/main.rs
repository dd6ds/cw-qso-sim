// src/main.rs  —  cw-qso-sim  entry point
mod audio;
mod config;
mod i18n;
mod keyer;
mod morse;
mod qso;
mod tui;

use anyhow::Result;
use clap::Parser;
use config::{AppConfig, Cli};
use morse::{Timing, Decoder};
use qso::{QsoEngine, QsoEvent};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::thread;
use std::time::Duration;

// ── Translated status messages ────────────────────────────────────────────────
struct StatusMsg {
    starting:          &'static str,
    demo_starting:     &'static str,
    transmitting:      &'static str,
    demo_transmitting: &'static str,
    demo_waiting:      &'static str,
    demo_preparing:    &'static str,
    demo_sending:      &'static str,
    listening:         &'static str,
    demo_complete:     &'static str,
    qso_complete:      &'static str,
    repeating:         &'static str,
}

impl StatusMsg {
    fn new(lang: &str) -> Self {
        match lang {
            "de" => Self {
                starting:          "Starte…",
                demo_starting:     "DEMO — SIM spielt das gesamte QSO…",
                transmitting:      "SIM sendet…",
                demo_transmitting: "DEMO: SIM sendet…",
                demo_waiting:      "DEMO: Warte auf SIM…",
                demo_preparing:    "DEMO: Antwort wird vorbereitet…",
                demo_sending:      "DEMO: Antwort wird gesendet…",
                listening:         "Warte auf dein Signal…",
                demo_complete:     "DEMO ABGESCHLOSSEN — ESC zum Beenden",
                qso_complete:      "QSO beendet — 73!",
                repeating:         "Letzte Sendung wird wiederholt…",
            },
            "fr" => Self {
                starting:          "Démarrage…",
                demo_starting:     "DÉMO — SIM joue le QSO complet…",
                transmitting:      "SIM émet…",
                demo_transmitting: "DÉMO: SIM émet…",
                demo_waiting:      "DÉMO: attente de fin SIM…",
                demo_preparing:    "DÉMO: préparation de la réponse…",
                demo_sending:      "DÉMO: envoi de la réponse…",
                listening:         "En attente de votre signal…",
                demo_complete:     "DÉMO TERMINÉE — ESC pour quitter",
                qso_complete:      "QSO terminé — 73!",
                repeating:         "Répétition de la dernière émission…",
            },
            "it" => Self {
                starting:          "Avvio…",
                demo_starting:     "DEMO — SIM riproduce il QSO completo…",
                transmitting:      "SIM trasmette…",
                demo_transmitting: "DEMO: SIM trasmette…",
                demo_waiting:      "DEMO: attesa fine SIM…",
                demo_preparing:    "DEMO: preparazione risposta…",
                demo_sending:      "DEMO: invio risposta…",
                listening:         "In attesa del tuo segnale…",
                demo_complete:     "DEMO COMPLETATA — ESC per uscire",
                qso_complete:      "QSO terminato — 73!",
                repeating:         "Ripetizione ultima trasmissione…",
            },
            _ => Self {  // English (default)
                starting:          "Starting…",
                demo_starting:     "DEMO — SIM will play the full QSO…",
                transmitting:      "SIM transmitting…",
                demo_transmitting: "DEMO: SIM transmitting…",
                demo_waiting:      "DEMO: waiting for SIM to finish…",
                demo_preparing:    "DEMO: preparing response…",
                demo_sending:      "DEMO: sending response…",
                listening:         "Listening for your key…",
                demo_complete:     "DEMO COMPLETE — Press ESC to exit",
                qso_complete:      "QSO complete — 73!",
                repeating:         "Repeating last TX…",
            },
        }
    }
}

// ── Shared UI state (passed to TUI draw) ─────────────────────────────────────
#[derive(Default, Clone)]
pub struct AppState {
    pub mycall:       String,
    pub sim_call:     String,
    pub sim_wpm:      u8,
    pub user_wpm:     u8,
    pub tone_hz:      u32,
    pub sim_log:      Vec<String>,
    pub user_decoded: String,
    pub current_code: String,
    pub status:       String,
    pub quit:         bool,
    pub text_mode:    bool,
    pub demo:         bool,
    pub no_decode:    bool,
}

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    // ── --help  ───────────────────────────────────────────────────────────────
    // Handled before config loading so that --lang <x> --help works without
    // a config file present.
    if cli.help {
        let lang_code = cli.lang.as_deref().unwrap_or("en");
        let i18n = i18n::I18n::new(lang_code);
        config::print_help(&i18n);
        return Ok(());
    }

    // ── --print-config  ───────────────────────────────────────────────────────
    if cli.print_config {
        print!("{}", config::DEFAULT_CONFIG_TOML);
        return Ok(());
    }

    // ── --write-config  ───────────────────────────────────────────────────────
    if cli.write_config {
        let path = AppConfig::write_default_config(&cli)?;
        println!("Config written to: {}", path.display());
        println!("Edit it to set your callsign, WPM, adapter, etc.");
        return Ok(());
    }

    // ── --list-ports  ─────────────────────────────────────────────────────────
    if cli.list_ports {
        let ports = keyer::list_ports();
        if ports.is_empty() {
            println!("No serial ports found.");
        } else {
            println!("Available serial ports:");
            for p in &ports { println!("  {p}"); }
        }
        return Ok(());
    }

    // ── --check-adapter  ──────────────────────────────────────────────────────
    if cli.check_adapter {
        let cfg = AppConfig::load(&cli)?;
        let port = if !cfg.midi_port.is_empty() { &cfg.midi_port } else { &cfg.port };
        let timeout = std::time::Duration::from_secs(10);

        // For --check-adapter: use --adapter if explicitly given on CLI,
        // otherwise always autodetect (ignore config file adapter setting).
        let adapter = if cli.adapter.is_some() {
            cfg.adapter
        } else {
            let detected = keyer::autodetect_adapter();
            log::info!("[check-adapter] autodetected: {:?}", detected);
            detected
        };

        let ok = match adapter {
            config::AdapterType::Vband => {
                #[cfg(feature = "keyer-vband")]
                { keyer::vband::check_adapter(timeout)? }
                #[cfg(not(feature = "keyer-vband"))]
                { println!("keyer-vband feature not compiled in."); false }
            }
            config::AdapterType::Attiny85 => {
                #[cfg(feature = "keyer-attiny85")]
                { keyer::attiny85::check_adapter(port, timeout)? }
                #[cfg(not(feature = "keyer-attiny85"))]
                { println!("keyer-attiny85 feature not compiled in."); false }
            }
            config::AdapterType::ArduinoNano => {
                #[cfg(feature = "keyer-nano")]
                { keyer::nano::check_adapter(port, "Arduino Nano (serial MIDI)", keyer::nano::BAUD_MIDI, timeout)? }
                #[cfg(not(feature = "keyer-nano"))]
                { println!("keyer-nano feature not compiled in."); false }
            }
            config::AdapterType::ArduinoUno => {
                #[cfg(feature = "keyer-nano")]
                { keyer::nano::check_adapter(port, "Arduino Uno (serial MIDI)", keyer::nano::BAUD_MIDI, timeout)? }
                #[cfg(not(feature = "keyer-nano"))]
                { println!("keyer-nano feature not compiled in."); false }
            }
            config::AdapterType::Esp32 => {
                #[cfg(feature = "keyer-nano")]
                { keyer::nano::check_adapter(port, "ESP32 (serial MIDI @ 115200)", keyer::nano::BAUD_ESP32, timeout)? }
                #[cfg(not(feature = "keyer-nano"))]
                { println!("keyer-nano feature not compiled in."); false }
            }
            config::AdapterType::Esp8266 => {
                #[cfg(feature = "keyer-nano")]
                { keyer::nano::check_adapter(port, "ESP8266 NodeMCU/Wemos (serial MIDI @ 115200)", keyer::nano::BAUD_ESP32, timeout)? }
                #[cfg(not(feature = "keyer-nano"))]
                { println!("keyer-nano feature not compiled in."); false }
            }
            config::AdapterType::WinKeyer => {
                #[cfg(feature = "keyer-winkeyer")]
                { keyer::winkeyer::check_adapter(port, timeout)? }
                #[cfg(not(feature = "keyer-winkeyer"))]
                { println!("keyer-winkeyer feature not compiled in."); false }
            }
            _ => {
                println!("No hardware adapter selected or detected.");
                println!("Supported: --adapter vband | attiny85 | arduino-nano | arduino-uno | esp32 | esp8266 | winkeyer");
                false
            }
        };
        std::process::exit(if ok { 0 } else { 1 });
    }

    // ── Load config ───────────────────────────────────────────────────────────
    let cfg = AppConfig::load(&cli)?;

    // ── i18n / status messages ────────────────────────────────────────────────
    let _lang = i18n::I18n::new(&cfg.language);
    let sm = StatusMsg::new(&cfg.language);

    // ── Timing — two independent clocks ──────────────────────────────────────
    // sim_wpm_shared : runtime-adjustable SIM speed; QRS/QRQ commands update it
    // user_timing    : drives the decoder (your keying speed)
    let sim_wpm_shared = Arc::new(AtomicU8::new(cfg.sim_wpm));
    let user_timing    = Timing::from_wpm(cfg.user_wpm);

    // ── Audio ─────────────────────────────────────────────────────────────────
    let audio = Arc::new(Mutex::new(
        audio::create_audio(cfg.tone_hz as f32, cfg.volume)
    ));

    // ── Keyer ─────────────────────────────────────────────────────────────────
    // For ATtiny85: --midi-port takes precedence over --port
    let keyer_port = if !cfg.midi_port.is_empty() { &cfg.midi_port } else { &cfg.port };
    let (keyer, is_keyboard, _windows_paddle) = keyer::create_keyer(cfg.adapter, keyer_port, cfg.paddle_mode, user_timing.dot, cfg.switch_paddle)?;

    // ── QSO engine ────────────────────────────────────────────────────────────
    // my_qso_serial starts at 1 and would increment across multiple QSOs in
    // a future multi-QSO session.  For now one process = one QSO.
    let my_qso_serial: u32 = 1;
    let mut engine = QsoEngine::new(&cfg, my_qso_serial);

    // ── Decoder (your keying) ─────────────────────────────────────────────────
    let mut decoder = Decoder::new(user_timing);

    // ── Shared app state ──────────────────────────────────────────────────────
    let state = Arc::new(Mutex::new(AppState {
        mycall:    cfg.mycall.clone(),
        sim_call:  engine.sim_callsign().to_string(),
        sim_wpm:   cfg.sim_wpm,
        user_wpm:  cfg.user_wpm,
        tone_hz:   cfg.tone_hz,
        status:    if cfg.demo { sm.demo_starting.into() }
                   else        { sm.starting.into() },
        text_mode: is_keyboard,
        demo:      cfg.demo,
        no_decode: cfg.no_decode,
        ..Default::default()
    }));

    // ── TUI ───────────────────────────────────────────────────────────────────
    #[cfg(feature = "tui")]
    let mut tui = tui::Tui::new(&cfg.language)?;

    // ── Spawn audio playback thread ───────────────────────────────────────────
    // The main thread drives the QSO; audio is dispatched via channel.
    // Playback holds the audio mutex for the full sequence — kept separate
    // from the sidetone path to avoid any blocking on the main loop.
    //
    // `audio_busy` is set to true by the main thread the moment it enqueues a
    // transmission, and cleared by the audio thread once play_sequence returns.
    // `tx_audio_done` carries a () signal back to the main loop so demo mode
    // knows exactly when the SIM has finished speaking.
    let audio_busy = Arc::new(AtomicBool::new(false));
    let audio_busy_audio = Arc::clone(&audio_busy);
    let (tx_audio,      rx_audio)      = std::sync::mpsc::channel::<String>();
    let (tx_audio_done, rx_audio_done) = std::sync::mpsc::channel::<()>();
    let audio_arc     = Arc::clone(&audio);
    let sim_wpm_audio = Arc::clone(&sim_wpm_shared);
    thread::spawn(move || {
        while let Ok(text) = rx_audio.recv() {
            let wpm    = sim_wpm_audio.load(Ordering::Relaxed);
            let timing = Timing::from_wpm(wpm);
            let seq    = morse::encode(&text, &timing);
            let mut a = audio_arc.lock().unwrap();
            let _ = a.play_sequence(&seq);
            drop(a); // release mutex before signalling
            audio_busy_audio.store(false, Ordering::Relaxed);
            let _ = tx_audio_done.send(());
        }
    });

    // ── Sidetone thread ───────────────────────────────────────────────────────
    // Uses its OWN lock attempt so it never blocks the main loop.
    // Sends (true=on, false=off).  The audio mutex may be held by the playback
    // thread, so we use try_lock and simply drop the sidetone command if busy.
    let (tx_sidetone, rx_sidetone) = std::sync::mpsc::channel::<bool>();
    let audio_st = Arc::clone(&audio);
    thread::spawn(move || {
        while let Ok(on) = rx_sidetone.recv() {
            // try_lock: if playback holds the mutex, skip sidetone silently
            if let Ok(mut a) = audio_st.try_lock() {
                if on { let _ = a.tone_on();  }
                else  { let _ = a.tone_off(); }
            }
        }
    });

    // ── Text-input state (adapter = text) ────────────────────────────────────
    // ── Keyboard text buffer (keyboard fallback mode) ─────────────────────────
    // When is_keyboard=true the user types characters directly.
    // Space commits the current word; Enter commits the word + signals end-of-over.
    // Backspace deletes the last character. Esc quits.
    // This bypasses the CW decoder entirely — hardware keyers use the decoder.
    let (tx_text, rx_text) = std::sync::mpsc::channel::<(String, bool)>();
    let mut kb_buf = String::new();  // accumulates typed chars between spaces/Enter

    // ── Hardware keyer polling thread ─────────────────────────────────────────
    // Sends (is_dah: bool, element_duration) to the main loop.
    // For the keyboard stub this thread runs but sends nothing (poll() = None).
    let (tx_key, rx_key) = std::sync::mpsc::channel::<(bool, std::time::Duration)>();
    let tx_key_thread = tx_key.clone();
    let mut keyer = keyer;
    let dot_dur   = user_timing.dot;
    thread::spawn(move || {
        loop {
            let ev = keyer.poll();
            use morse::decoder::PaddleEvent::*;
            match ev {
                DitDown => { let _ = tx_key_thread.send((false, dot_dur)); }
                DahDown => { let _ = tx_key_thread.send((true,  dot_dur * 3)); }
                _ => {}
            }
            thread::sleep(Duration::from_millis(2));
        }
    });
    // ── Main loop ─────────────────────────────────────────────────────────────
    let tick = Duration::from_millis(10);
    // Accumulates decoded chars across ticks.
    // Submitted to the QSO engine only when a word-gap (space) is decoded,
    // meaning the user finished transmitting a word.
    // The engine decides whether the content is sufficient to advance.
    let mut user_tx_acc = String::new();

    // ── Demo mode state ───────────────────────────────────────────────────────
    // Two-stage pipeline so the user reply is only sent after the SIM finishes
    // its CW transmission:
    //
    //   Stage 1 — demo_queued_response: Option<String>
    //     Set when WaitingForUser fires.  Waits here until rx_audio_done fires.
    //
    //   Stage 2 — demo_pending: Option<(Instant, String)>
    //     Moved from stage 1 once audio is done.  Fires after a short
    //     "reaction time" delay (600 ms) to feel human.
    let mut demo_queued_response: Option<String>                      = None;
    let mut demo_pending:         Option<(std::time::Instant, String)> = None;
    // Set to true once the QSO completes in demo mode — keeps the TUI alive
    // until the user presses ESC.
    let mut demo_complete: bool = false;

    'main: loop {
        // ── Single crossterm event reader ─────────────────────────────────────
        // ALL events are read here — never in any other thread.
        #[cfg(feature = "tui")]
        {
            use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
            while event::poll(Duration::from_millis(0))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Release {
                        // Release events are not used — all paddle state comes from
                        // VBandKeyer::poll() (GetAsyncKeyState on Windows, hidapi elsewhere).
                        continue;
                    }

                    // Escape or Ctrl+C always quit
                    if k.code == KeyCode::Esc
                        || (k.code == KeyCode::Char('c')
                            && k.modifiers.contains(KeyModifiers::CONTROL))
                    {
                        break 'main;
                    }

                    if is_keyboard {
                        // ── Text input mode ────────────────────────────────
                        // Type characters normally; Space = commit word,
                        // Enter = commit word + end-of-over (like pressing K).
                        // Q is a regular letter here — use Esc to quit.
                        match k.code {
                            KeyCode::Backspace => { kb_buf.pop(); }
                            KeyCode::Enter => {
                                let word = kb_buf.trim().to_string();
                                kb_buf.clear();
                                if !word.is_empty() {
                                    let _ = tx_text.send((word, true));
                                }
                            }
                            KeyCode::Char(' ') => {
                                let word = kb_buf.trim().to_string();
                                kb_buf.clear();
                                if !word.is_empty() {
                                    let _ = tx_text.send((word, false));
                                }
                            }
                            KeyCode::Char(c) => {
                                kb_buf.push(c.to_ascii_uppercase());
                            }
                            _ => {}
                        }
                    } else {
                        // ── Hardware keyer mode ────────────────────────────
                        if k.code == KeyCode::Char('q') || k.code == KeyCode::Char('Q') {
                            break 'main;
                        }
                    }
                }
            }
        }

        // Drain keyer events → sidetone + decoder
        while let Ok((is_dah, el_dur)) = rx_key.try_recv() {
            log::debug!("[main-loop] rx_key received: is_dah={} el_dur={:?}", is_dah, el_dur);
            if cfg.sidetone {
                let tx_st = tx_sidetone.clone();
                thread::spawn(move || {
                    let _ = tx_st.send(true);
                    thread::sleep(el_dur);
                    let _ = tx_st.send(false);
                });
            }
            // Pass el_dur so the decoder measures char_gap from the element END
            log::debug!("[main-loop] push_element: is_dah={} el_dur={:?}", is_dah, el_dur);
            decoder.push_element(is_dah, el_dur);
        }

        // Tick decoder — always run so the QSO engine can advance;
        // only update the UI display when --no-decode is not set.
        let mut word_boundary = false;
        if let Some(new_chars) = decoder.tick() {
            if new_chars.contains(' ') { word_boundary = true; }
            user_tx_acc.push_str(&new_chars);
            if !cfg.no_decode {
                let mut st = state.lock().unwrap();
                st.user_decoded.push_str(&new_chars);
                if st.user_decoded.len() > 200 {
                    let trim = st.user_decoded.len() - 200;
                    st.user_decoded = st.user_decoded[trim..].to_string();
                }
            }
        }

        // Text-adapter injection — bypass CW decoder entirely
        let mut text_end_of_over = false;
        while let Ok((word, eoo)) = rx_text.try_recv() {
            let entry = format!("{word} ");
            user_tx_acc.push_str(&entry);
            {
                let mut st = state.lock().unwrap();
                st.user_decoded.push_str(&entry);
                st.current_code.clear();
                if st.user_decoded.len() > 200 {
                    let trim = st.user_decoded.len() - 200;
                    st.user_decoded = st.user_decoded[trim..].to_string();
                }
            }
            word_boundary = true;
            if eoo { text_end_of_over = true; }
        }

        // Update current_code display (suppressed when --no-decode is set)
        if !cfg.no_decode {
            let mut st = state.lock().unwrap();
            st.current_code = if is_keyboard {
                kb_buf.clone()  // show what's being typed
            } else {
                decoder.current_code().to_string()  // show CW elements being keyed
            };
        }

        // ── Demo: audio-done → stage 2 ────────────────────────────────────────
        // Drain all done signals from the audio thread.  When we have a queued
        // response waiting (stage 1), promote it to stage 2 (timed delay).
        if cfg.demo && !demo_complete {
            while rx_audio_done.try_recv().is_ok() {
                if let Some(resp) = demo_queued_response.take() {
                    let fire_at = std::time::Instant::now()
                        + Duration::from_millis(600);
                    demo_pending = Some((fire_at, resp));
                    let mut st = state.lock().unwrap();
                    st.status = sm.demo_preparing.into();
                }
            }
        }

        // ── Demo auto-injector ─────────────────────────────────────────────────
        // When a demo response has been scheduled and its timer fires, inject it
        // directly into the user input accumulator so the engine can advance.
        if cfg.demo && !demo_complete {
            if let Some((ref fire_at, ref resp)) = demo_pending {
                if std::time::Instant::now() >= *fire_at {
                    let resp = resp.clone();
                    // Show in the YOUR INPUT panel
                    {
                        let mut st = state.lock().unwrap();
                        st.user_decoded.push_str(&resp);
                        st.user_decoded.push(' ');
                        if st.user_decoded.len() > 200 {
                            let trim = st.user_decoded.len() - 200;
                            st.user_decoded = st.user_decoded[trim..].to_string();
                        }
                        st.status = sm.demo_sending.into();
                    }
                    user_tx_acc     = resp;
                    word_boundary   = true;
                    text_end_of_over = true;
                    demo_pending    = None;
                }
            }
        }

        // QSO engine tick — accumulate the full over across word boundaries.
        // Only submit to the engine (and clear) when an end-of-over prosign
        // is received — or Enter in text-adapter mode.
        //
        // End-of-over markers, covering both text-mode and CW-decoder output:
        //   "K"   — letter K (.-),              decoded as 'K'
        //   "BK"  — prosign <BK> (-...-.-),     decoded as ' ' (word-gap)
        //   "AR"  — prosign <AR> (.-.-.),        decoded as '+' by CW decoder
        //   "KN"  — prosign <KN> (-.--.) ,       decoded as '(' by CW decoder
        //
        // The CW decoder emits '+' for <AR> and '(' for <KN>, so we must
        // accept those single-char forms in addition to the text-mode strings.
        let end_of_over = text_end_of_over || if word_boundary {
            let last_word = user_tx_acc.trim().to_uppercase();
            match last_word.split_whitespace().last() {
                Some("K" | "BK" | "AR" | "KN") => true,
                // CW-decoder prosign equivalents: <AR> → '+', <KN> → '('
                Some("+" | "(") => true,
                _ => false,
            }
        } else {
            false
        };

        // ── QRS / QRQ speed adjustment ─────────────────────────────────────────
        // Only fires at end-of-over — never on mid-over word boundaries — so a
        // callsign over like "SM5XY DE DD6DS K" is never mistaken for QRS.
        // QRS/QRQ is STRIPPED from the over; remaining words (callsign, PSE, K)
        // are still passed to the QSO engine so the QSO advances normally.
        //
        // Accepted patterns (any order, with or without surrounding words):
        //   "QRS K"                  "QRQ K"
        //   "PSE QRS K"              "PSE QRQ K"
        //   "QRS PSE K"              "QRQ PSE K"
        //   "SM5XY DE DD6DS QRS K"   "SM5XY DE DD6DS QRQ K"
        //   "SM5XY DE DD6DS PSE QRS K"
        let mut input_to_pass = if end_of_over {
            user_tx_acc.trim().to_uppercase()
        } else {
            String::new()
        };

        if !input_to_pass.is_empty() {
            let has_qrs = input_to_pass.split_whitespace().any(|w| w == "QRS");
            let has_qrq = !has_qrs && input_to_pass.split_whitespace().any(|w| w == "QRQ");
            if has_qrs || has_qrq {
                let filter_word = if has_qrs { "QRS" } else { "QRQ" };
                let new_wpm = if has_qrs {
                    sim_wpm_shared.load(Ordering::Relaxed).saturating_sub(3).max(5)
                } else {
                    sim_wpm_shared.load(Ordering::Relaxed).saturating_add(3).min(50)
                };
                sim_wpm_shared.store(new_wpm, Ordering::Relaxed);
                state.lock().unwrap().sim_wpm = new_wpm;
                // Strip QRS/QRQ so the rest of the over reaches the engine
                let stripped: Vec<&str> = input_to_pass.split_whitespace()
                    .filter(|&w| w != filter_word).collect();
                input_to_pass = stripped.join(" ");

                // Only send an explicit QRS/QRQ ack when the over contained
                // nothing else meaningful (standalone speed command).
                // If the user also sent a callsign or exchange content the
                // engine will reply at the new — already slower — speed,
                // which serves as the implicit acknowledgment.
                let filler = ["K", "BK", "AR", "KN", "+", "(", "PSE", "DE"];
                let has_content = input_to_pass.split_whitespace()
                    .any(|w| !filler.contains(&w));
                if !has_content {
                    let ack = if has_qrs { "QRS QRS" } else { "QRQ QRQ" };
                    {
                        let mut st = state.lock().unwrap();
                        if !cfg.no_decode {
                            st.sim_log.push(ack.to_string());
                            if st.sim_log.len() > 50 { st.sim_log.remove(0); }
                        }
                        st.status = sm.transmitting.into();
                    }
                    audio_busy.store(true, Ordering::Relaxed);
                    let _ = tx_audio.send(ack.to_string());
                }
            }
        }

        let event = engine.tick(&input_to_pass);
        if end_of_over {
            user_tx_acc.clear();
        }

        match event {
            Some(QsoEvent::SimTransmit(text)) => {
                {
                    let mut st = state.lock().unwrap();
                    if !cfg.no_decode {
                        st.sim_log.push(text.clone());
                        if st.sim_log.len() > 50 { st.sim_log.remove(0); }
                    }
                    st.status = if cfg.demo { sm.demo_transmitting.into() }
                                else        { sm.transmitting.into() };
                }
                // Mark audio as busy BEFORE sending to the channel so that
                // WaitingForUser (which fires on the very next tick) sees the
                // correct state and knows to wait for the done signal.
                audio_busy.store(true, Ordering::Relaxed);
                let _ = tx_audio.send(text);
            }
            Some(QsoEvent::WaitingForUser) => {
                if cfg.demo && !demo_complete {
                    // Queue a response only once per waiting phase.
                    if demo_queued_response.is_none() && demo_pending.is_none() {
                        if let Some(resp) = engine.demo_response() {
                            if audio_busy.load(Ordering::Relaxed) {
                                // SIM is still transmitting — park the response
                                // here until rx_audio_done fires (stage 1).
                                demo_queued_response = Some(resp);
                                let mut st = state.lock().unwrap();
                                st.status = sm.demo_waiting.into();
                            } else {
                                // No audio in flight (e.g. ISendCq before SIM
                                // has sent anything) — go straight to stage 2.
                                let fire_at = std::time::Instant::now()
                                    + Duration::from_millis(600);
                                demo_pending = Some((fire_at, resp));
                                let mut st = state.lock().unwrap();
                                st.status = sm.demo_preparing.into();
                            }
                        }
                    }
                } else {
                    let mut st = state.lock().unwrap();
                    st.status = sm.listening.into();
                }
            }
            Some(QsoEvent::QsoComplete) => {
                if cfg.demo {
                    // Keep the TUI alive — user reads the log then presses ESC
                    demo_complete = true;
                    let mut st = state.lock().unwrap();
                    st.status = sm.demo_complete.into();
                } else {
                    {
                        let mut st = state.lock().unwrap();
                        st.status = sm.qso_complete.into();
                    }
                    // Draw final state, then wait a moment
                    #[cfg(feature = "tui")]
                    {
                        let st = state.lock().unwrap().clone();
                        tui.draw(&st)?;
                    }
                    thread::sleep(Duration::from_secs(3));
                    break 'main;
                }
            }
            Some(QsoEvent::RepeatLast) => {
                let mut st = state.lock().unwrap();
                st.status = sm.repeating.into();
            }
            None => {}
        }

        // Draw TUI
        #[cfg(feature = "tui")]
        {
            let st = state.lock().unwrap().clone();
            tui.draw(&st)?;
        }

        thread::sleep(tick);
    }

    // ── Cleanup ───────────────────────────────────────────────────────────────
    #[cfg(feature = "tui")]
    tui.cleanup();

    println!("\n73 de cw-qso-sim! Good luck with the pile-ups.\n");
    Ok(())
}
