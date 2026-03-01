// src/config.rs  —  Runtime configuration (CLI + TOML)
use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The example config is embedded directly in the binary at compile time.
/// Users can write it out with:  cw-qso-sim --write-config
pub const DEFAULT_CONFIG_TOML: &str = include_str!("../config.toml.example");

// ── CLI ───────────────────────────────────────────────────────────────────────
#[derive(Parser, Debug)]
#[command(
    name        = "cw-qso-sim",
    about       = "Morse Code QSO Simulator  |  DD6DS",
    version,
)]
pub struct Cli {
    /// Config file path (default: ~/.config/cw-qso-sim/config.toml)
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Your callsign (e.g. DD6DS)
    #[arg(long)]
    pub mycall: Option<String>,

    /// Simulator TX speed in WPM (default: 25)
    #[arg(long)]
    pub sim_wpm: Option<u8>,

    /// Your keying speed in WPM — controls decoder timing (default: 18)
    #[arg(long)]
    pub user_wpm: Option<u8>,

    /// Sidetone frequency Hz
    #[arg(long)]
    pub tone: Option<u32>,

    /// Who starts the QSO: me | sim
    #[arg(long)]
    pub who_starts: Option<WhoStarts>,

    /// QSO style: ragchew | contest | dx-pileup | darc-cw-contest | mwc-contest | cwt-contest | random
    #[arg(long)]
    pub style: Option<QsoStyle>,

    /// Your operator name for cwt_contest exchange (e.g. HANS)
    #[arg(long)]
    pub cwt_name: Option<String>,

    /// Your CWT member number or state/country for cwt_contest (e.g. 1234 or DL)
    #[arg(long)]
    pub cwt_nr: Option<String>,

    /// Your DARC DOK for darc-cw-contest (e.g. P53).  Use NM if not a DARC member.
    #[arg(long)]
    pub my_dok: Option<String>,

    /// Keyer adapter: auto | vband | attiny85 | arduino-nano | arduino-uno | esp32 | esp8266 | winkeyer | keyboard
    #[arg(long)]
    pub adapter: Option<AdapterType>,

    /// Serial port for arduino-nano, arduino-uno, esp32, esp8266 or winkeyer (e.g. /dev/ttyUSB0, COM3)
    #[arg(long)]
    pub port: Option<String>,

    /// MIDI port name or substring for ATtiny85 adapter (overrides --port)
    #[arg(long)]
    pub midi_port: Option<String>,

    /// Paddle mode: iambic_a | iambic_b | straight
    #[arg(long)]
    pub paddle_mode: Option<PaddleMode>,

    /// Swap DIT and DAH paddles
    #[arg(long, action)]
    pub switch_paddle: bool,

    /// UI language: en | de | fr | it
    #[arg(long)]
    pub lang: Option<String>,

    /// List available HID/serial keyer devices and exit
    #[arg(long, action)]
    pub list_ports: bool,

    /// Test the configured adapter: press DIT then DAH when prompted
    #[arg(long, action)]
    pub check_adapter: bool,

    /// Write the built-in default config.toml to the config path and exit.
    /// Use --config <PATH> to write to a custom location.
    #[arg(long, action)]
    pub write_config: bool,

    /// Print the built-in default config.toml to stdout and exit
    #[arg(long, action)]
    pub print_config: bool,

    /// Demo mode: play a complete QSO automatically (no keyer needed), then
    /// wait for ESC to exit.  Useful to preview a contest style before practising.
    #[arg(long, action)]
    pub demo: bool,
}

// ── Enums shared across CLI + TOML ────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum WhoStarts { Me, Sim }

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum QsoStyle { Ragchew, Contest, DxPileup, DarcCwContest, MwcContest, CwtContest, Random }

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum AdapterType {
    /// Auto-detect hardware; fall back to keyboard if none found
    Auto,
    /// VBand USB HID paddle (Linux/Windows only; not available in this build on MacOS)
    #[cfg_attr(not(feature = "keyer-vband"), value(skip))]
    Vband,
    /// ATtiny85 MIDI paddle
    #[cfg_attr(not(feature = "keyer-attiny85"), value(skip))]
    Attiny85,
    /// Arduino Nano serial-MIDI paddle (31250 baud; autodetects CH340/FT232/ATmega16U2)
    #[cfg_attr(not(feature = "keyer-nano"), value(skip))]
    ArduinoNano,
    /// Arduino Uno serial-MIDI paddle (31250 baud; autodetects ATmega16U2/CH340)
    #[cfg_attr(not(feature = "keyer-nano"), value(skip))]
    ArduinoUno,
    /// ESP32 serial-MIDI paddle (115200 baud; requires --port <device>)
    #[cfg_attr(not(feature = "keyer-nano"), value(skip))]
    #[value(name = "esp32")]
    Esp32,
    /// ESP8266 serial-MIDI paddle (115200 baud; NodeMCU / Wemos D1 Mini — requires --port <device>)
    #[cfg_attr(not(feature = "keyer-nano"), value(skip))]
    #[value(name = "esp8266")]
    Esp8266,
    /// K1EL WinKeyer USB/Serial (WK2/WK3) — requires --port <device>
    #[cfg_attr(not(feature = "keyer-winkeyer"), value(skip))]
    #[value(name = "winkeyer")]
    WinKeyer,
    /// Keyboard text-input mode (type callsigns, Space=word, Enter=over)
    Keyboard,
    /// Hidden — text-mode input (legacy alias for keyboard)
    #[value(skip)]
    Text,
    /// Hidden — same as keyboard
    #[value(skip)]
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum PaddleMode { IambicA, IambicB, Straight }

// ── TOML file structure ───────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileConfig {
    pub general: Option<GeneralCfg>,
    pub morse:   Option<MorseCfg>,
    pub keyer:   Option<KeyerCfg>,
    pub qso:     Option<QsoCfg>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralCfg {
    pub language:   Option<String>,
    pub who_starts: Option<WhoStarts>,
    pub mycall:     Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorseCfg {
    /// Simulator TX speed (WPM)
    pub sim_wpm:           Option<u8>,
    /// Your keying speed — decoder timing (WPM)
    pub user_wpm:          Option<u8>,
    /// Farnsworth effective WPM applied to user decoder
    pub farnsworth_wpm:    Option<u8>,
    pub tone_hz:           Option<u32>,
    pub volume:            Option<f32>,
    pub sidetone:          Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyerCfg {
    pub adapter:       Option<AdapterType>,
    pub mode:          Option<PaddleMode>,
    pub port:          Option<String>,
    pub switch_paddle: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QsoCfg {
    pub style:        Option<QsoStyle>,
    pub min_delay_ms: Option<u64>,
    pub max_delay_ms: Option<u64>,
    pub typo_rate:    Option<f64>,
    pub cwt_name:     Option<String>,
    pub cwt_nr:       Option<String>,
    pub my_dok:       Option<String>,
}

// ── Resolved / merged config ──────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub mycall:         String,
    pub language:       String,
    pub who_starts:     WhoStarts,
    /// Simulator TX speed
    pub sim_wpm:        u8,
    /// User keying / decoder speed
    pub user_wpm:       u8,
    pub farnsworth_wpm: u8,
    pub tone_hz:        u32,
    pub volume:         f32,
    pub sidetone:       bool,
    pub adapter:        AdapterType,
    pub paddle_mode:    PaddleMode,
    pub switch_paddle:  bool,
    pub port:           String,
    pub midi_port:      String,
    pub qso_style:      QsoStyle,
    pub min_delay_ms:   u64,
    pub max_delay_ms:   u64,
    pub typo_rate:      f64,
    /// User's operator name for CWT contest exchange
    pub cwt_name:       String,
    /// Demo mode: play QSO automatically, no keyer input required
    pub demo:           bool,
    /// User's CWT member number or state/country (e.g. "1234" or "DL")
    pub cwt_nr:         String,
    /// User's own DARC DOK for darc-cw-contest (e.g. "P53", or "NM" for non-members)
    pub my_dok:         String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mycall:         "N0CALL".into(),
            language:       "en".into(),
            who_starts:     WhoStarts::Sim,
            sim_wpm:        25,
            user_wpm:       18,
            farnsworth_wpm: 0,
            tone_hz:        620,
            volume:         0.7,
            sidetone:       true,
            adapter:        AdapterType::Auto,
            paddle_mode:    PaddleMode::IambicA,
            switch_paddle:  false,
            port:           String::new(),
            midi_port:      String::new(),
            qso_style:      QsoStyle::Ragchew,
            min_delay_ms:   800,
            max_delay_ms:   2500,
            typo_rate:      0.05,
            cwt_name:       "OP".into(),
            cwt_nr:         "NM".into(),
            my_dok:         "NM".into(),
            demo:           false,
        }
    }
}

// ── Config loader ─────────────────────────────────────────────────────────────
impl AppConfig {
    /// Write the embedded default config to disk.
    /// Returns the path it was written to.
    pub fn write_default_config(cli: &Cli) -> Result<PathBuf> {
        let path = cli.config.clone().unwrap_or_else(default_config_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Creating config directory {:?}", parent))?;
        }
        std::fs::write(&path, DEFAULT_CONFIG_TOML)
            .with_context(|| format!("Writing config to {:?}", path))?;
        Ok(path)
    }

    pub fn load(cli: &Cli) -> Result<Self> {
        let mut cfg = Self::default();

        // 1. Load TOML file
        let path = cli.config.clone().unwrap_or_else(default_config_path);
        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("Reading config {:?}", path))?;
            let fc: FileConfig = toml::from_str(&raw)
                .with_context(|| format!("Parsing config {:?}", path))?;
            cfg.apply_file(&fc);
        } else {
            eprintln!(
                "No config file found at {}\n  \
                 → Run `cw-qso-sim --write-config` to create one, then set your callsign.",
                path.display()
            );
        }

        // 2. Apply CLI overrides
        cfg.apply_cli(cli);
        Ok(cfg)
    }

    fn apply_file(&mut self, fc: &FileConfig) {
        if let Some(g) = &fc.general {
            if let Some(v) = &g.language   { self.language   = v.clone(); }
            if let Some(v) = &g.who_starts { self.who_starts = *v; }
            if let Some(v) = &g.mycall     { self.mycall     = v.clone(); }
        }
        if let Some(m) = &fc.morse {
            if let Some(v) = m.sim_wpm         { self.sim_wpm        = v; }
            if let Some(v) = m.user_wpm        { self.user_wpm       = v; }
            if let Some(v) = m.farnsworth_wpm  { self.farnsworth_wpm = v; }
            if let Some(v) = m.tone_hz         { self.tone_hz        = v; }
            if let Some(v) = m.volume          { self.volume         = v; }
            if let Some(v) = m.sidetone        { self.sidetone       = v; }
        }
        if let Some(k) = &fc.keyer {
            if let Some(v) = k.adapter       { self.adapter       = v; }
            if let Some(v) = k.mode          { self.paddle_mode   = v; }
            if let Some(v) = &k.port         { self.port          = v.clone(); }
            if let Some(v) = k.switch_paddle { self.switch_paddle = v; }
        }
        if let Some(q) = &fc.qso {
            if let Some(v) = q.style        { self.qso_style    = v; }
            if let Some(v) = q.min_delay_ms { self.min_delay_ms = v; }
            if let Some(v) = q.max_delay_ms { self.max_delay_ms = v; }
            if let Some(v) = q.typo_rate    { self.typo_rate    = v; }
            if let Some(v) = &q.cwt_name    { self.cwt_name     = v.clone(); }
            if let Some(v) = &q.cwt_nr      { self.cwt_nr       = v.clone(); }
            if let Some(v) = &q.my_dok      { self.my_dok       = v.clone(); }
        }
    }

    fn apply_cli(&mut self, cli: &Cli) {
        if let Some(v) = &cli.mycall     { self.mycall      = v.clone(); }
        if let Some(v) = cli.sim_wpm     { self.sim_wpm     = v; }
        if let Some(v) = cli.user_wpm    { self.user_wpm    = v; }
        if let Some(v) = cli.tone        { self.tone_hz     = v; }
        if let Some(v) = cli.who_starts  { self.who_starts  = v; }
        if let Some(v) = cli.style       { self.qso_style   = v; }
        if let Some(v) = cli.adapter     { self.adapter     = v; }
        if let Some(v) = &cli.port       { self.port        = v.clone(); }
        if let Some(v) = &cli.midi_port  { self.midi_port   = v.clone(); }
        if let Some(v) = cli.paddle_mode { self.paddle_mode = v; }
        if cli.switch_paddle             { self.switch_paddle = true; }
        if let Some(v) = &cli.lang       { self.language    = v.clone(); }
        if let Some(v) = &cli.cwt_name   { self.cwt_name    = v.clone(); }
        if let Some(v) = &cli.cwt_nr     { self.cwt_nr      = v.clone(); }
        if let Some(v) = &cli.my_dok     { self.my_dok      = v.clone(); }
        if cli.demo                      { self.demo        = true; }
    }
}

fn default_config_path() -> PathBuf {
    dirs_next().join("cw-qso-sim").join("config.toml")
}

fn dirs_next() -> PathBuf {
    if let Ok(v) = std::env::var("XDG_CONFIG_HOME") { return PathBuf::from(v); }
    if let Ok(v) = std::env::var("APPDATA")          { return PathBuf::from(v); }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    PathBuf::from(home).join(".config")
}
