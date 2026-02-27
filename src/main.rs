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
use std::thread;
use std::time::Duration;

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
}

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

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
            _ => {
                println!("No hardware adapter selected or detected.");
                println!("Use --adapter vband or --adapter attiny85");
                false
            }
        };
        std::process::exit(if ok { 0 } else { 1 });
    }

    // ── Load config ───────────────────────────────────────────────────────────
    let cfg = AppConfig::load(&cli)?;

    // ── i18n ──────────────────────────────────────────────────────────────────
    let _lang = i18n::I18n::new(&cfg.language);

    // ── Timing — two independent clocks ──────────────────────────────────────
    // sim_timing  : drives audio playback of the simulator's CW
    // user_timing : drives the decoder (your keying speed)
    let sim_timing = if cfg.farnsworth_wpm > 0 {
        Timing::farnsworth(cfg.sim_wpm, cfg.farnsworth_wpm)
    } else {
        Timing::from_wpm(cfg.sim_wpm)
    };
    let user_timing = Timing::from_wpm(cfg.user_wpm);

    // ── Audio ─────────────────────────────────────────────────────────────────
    let audio = Arc::new(Mutex::new(
        audio::create_audio(cfg.tone_hz as f32, cfg.volume)
    ));

    // ── Keyer ─────────────────────────────────────────────────────────────────
    // For ATtiny85: --midi-port takes precedence over --port
    let keyer_port = if !cfg.midi_port.is_empty() { &cfg.midi_port } else { &cfg.port };
    let (keyer, is_keyboard, _windows_paddle) = keyer::create_keyer(cfg.adapter, keyer_port, cfg.paddle_mode, user_timing.dot, cfg.switch_paddle)?;

    // ── QSO engine ────────────────────────────────────────────────────────────
    let mut engine = QsoEngine::new(&cfg);

    // ── Decoder (your keying) ─────────────────────────────────────────────────
    let mut decoder = Decoder::new(user_timing);

    // ── Shared app state ──────────────────────────────────────────────────────
    let state = Arc::new(Mutex::new(AppState {
        mycall:    cfg.mycall.clone(),
        sim_call:  engine.sim_callsign().to_string(),
        sim_wpm:   cfg.sim_wpm,
        user_wpm:  cfg.user_wpm,
        tone_hz:   cfg.tone_hz,
        status:    "Starting…".into(),
        text_mode: is_keyboard,
        ..Default::default()
    }));

    // ── TUI ───────────────────────────────────────────────────────────────────
    #[cfg(feature = "tui")]
    let mut tui = tui::Tui::new(&cfg.language)?;

    // ── Spawn audio playback thread ───────────────────────────────────────────
    // The main thread drives the QSO; audio is dispatched via channel.
    // Playback holds the audio mutex for the full sequence — kept separate
    // from the sidetone path to avoid any blocking on the main loop.
    let (tx_audio, rx_audio) = std::sync::mpsc::channel::<String>();
    let audio_arc    = Arc::clone(&audio);
    let sim_timing_c = sim_timing;
    thread::spawn(move || {
        while let Ok(text) = rx_audio.recv() {
            let seq = morse::encode(&text, &sim_timing_c);
            let mut a = audio_arc.lock().unwrap();
            let _ = a.play_sequence(&seq);
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

        // Tick decoder
        let mut word_boundary = false;
        if let Some(new_chars) = decoder.tick() {
            if new_chars.contains(' ') { word_boundary = true; }
            user_tx_acc.push_str(&new_chars);
            let mut st = state.lock().unwrap();
            st.user_decoded.push_str(&new_chars);
            if st.user_decoded.len() > 200 {
                let trim = st.user_decoded.len() - 200;
                st.user_decoded = st.user_decoded[trim..].to_string();
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

        // Update current_code display
        {
            let mut st = state.lock().unwrap();
            st.current_code = if is_keyboard {
                kb_buf.clone()  // show what's being typed
            } else {
                decoder.current_code().to_string()  // show CW elements being keyed
            };
        }

        // QSO engine tick — accumulate the full over across word boundaries.
        // Only submit to the engine (and clear) when an end-of-over prosign
        // is received: K, BK, AR, KN — or Enter in text-adapter mode.
        let end_of_over = text_end_of_over || if word_boundary {
            let last_word = user_tx_acc.trim().to_uppercase();
            matches!(last_word.split_whitespace().last(), Some("K" | "BK" | "AR" | "KN"))
        } else {
            false
        };

        let input_to_pass = if end_of_over {
            user_tx_acc.trim().to_uppercase()
        } else {
            String::new()
        };
        let event = engine.tick(&input_to_pass);
        if end_of_over {
            user_tx_acc.clear();
        }

        match event {
            Some(QsoEvent::SimTransmit(text)) => {
                {
                    let mut st = state.lock().unwrap();
                    st.sim_log.push(text.clone());
                    if st.sim_log.len() > 50 { st.sim_log.remove(0); }
                    st.status = "SIM transmitting…".into();
                }
                let _ = tx_audio.send(text);
            }
            Some(QsoEvent::WaitingForUser) => {
                let mut st = state.lock().unwrap();
                st.status = "Listening for your key…".into();
            }
            Some(QsoEvent::QsoComplete) => {
                {
                    let mut st = state.lock().unwrap();
                    st.status = "QSO complete — 73!".into();
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
            Some(QsoEvent::RepeatLast) => {
                let mut st = state.lock().unwrap();
                st.status = "Repeating last TX…".into();
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
