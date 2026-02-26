// src/i18n/mod.rs  —  Multi-language string tables
use std::collections::HashMap;

/// All user-visible strings referenced by key
pub trait Lang: Send + Sync {
    fn get<'a>(&self, key: &'a str) -> &'a str;
    fn name(&self) -> &str;
}

/// Central i18n registry
pub struct I18n {
    inner: Box<dyn Lang>,
}

impl I18n {
    pub fn new(lang: &str) -> Self {
        let inner: Box<dyn Lang> = match lang {
            "de" => Box::new(De::new()),
            "fr" => Box::new(Fr::new()),
            "it" => Box::new(It::new()),
            _    => Box::new(En::new()),
        };
        Self { inner }
    }
    pub fn t<'a>(&self, key: &'a str) -> &'a str { self.inner.get(key) }
    pub fn lang_name(&self) -> &str    { self.inner.name() }
}

// ── Helper macro ──────────────────────────────────────────────────────────────
macro_rules! lang_map {
    ($name:ident, $display:literal, [ $( $k:literal => $v:literal ),* $(,)? ]) => {
        pub struct $name(HashMap<&'static str, &'static str>);
        impl $name {
            pub fn new() -> Self {
                let mut m = HashMap::new();
                $( m.insert($k, $v); )*
                Self(m)
            }
        }
        impl Lang for $name {
            fn get<'a>(&self, key: &'a str) -> &'a str {
                self.0.get(key).copied().unwrap_or(key)
            }
            fn name(&self) -> &str { $display }
        }
    };
}
// ── English ───────────────────────────────────────────────────────────────────
lang_map!(En, "English", [
    "app.title"          => "CW QSO Simulator",
    "app.quit"           => "Press Q to quit",
    "menu.wpm"           => "Speed (WPM)",
    "menu.tone"          => "Tone (Hz)",
    "menu.adapter"       => "Keyer adapter",
    "menu.whoStarts"     => "Who starts QSO",
    "menu.style"         => "QSO style",
    "menu.lang"          => "Language",
    "label.tx"           => "SIM TX",
    "label.rx"           => "YOUR RX",
    "label.decoded"      => "Decoded",
    "label.status"       => "Status",
    "status.waiting"     => "Waiting for your signal…",
    "status.listening"   => "Listening…",
    "status.transmitting"=> "Transmitting…",
    "status.qso_end"     => "QSO ended  —  73 de SIM",
    "err.no_port"        => "No serial port found. Use --port to specify one.",
    "whoStarts.me"       => "I start (send CQ)",
    "whoStarts.sim"      => "Simulator starts (sends CQ)",
    "style.ragchew"      => "Rag-chew",
    "style.contest"      => "Contest",
    "style.dx"           => "DX pile-up",
    "style.random"       => "Random",
]);

// ── German ────────────────────────────────────────────────────────────────────
lang_map!(De, "Deutsch", [
    "app.title"          => "CW QSO Simulator",
    "app.quit"           => "Q drücken zum Beenden",
    "menu.wpm"           => "Geschwindigkeit (WPM)",
    "menu.tone"          => "Ton (Hz)",
    "menu.adapter"       => "Keyer-Adapter",
    "menu.whoStarts"     => "Wer beginnt das QSO",
    "menu.style"         => "QSO-Stil",
    "menu.lang"          => "Sprache",
    "label.tx"           => "SIM SENDET",
    "label.rx"           => "DEIN EMPFANG",
    "label.decoded"      => "Dekodiert",
    "label.status"       => "Status",
    "status.waiting"     => "Warte auf dein Signal…",
    "status.listening"   => "Höre zu…",
    "status.transmitting"=> "Sende…",
    "status.qso_end"     => "QSO beendet  —  73 de SIM",
    "err.no_port"        => "Kein serieller Port gefunden. --port verwenden.",
    "whoStarts.me"       => "Ich beginne (CQ senden)",
    "whoStarts.sim"      => "Simulator beginnt (sendet CQ)",
    "style.ragchew"      => "Rag-Chew",
    "style.contest"      => "Contest",
    "style.dx"           => "DX Pile-Up",
    "style.random"       => "Zufällig",
]);

// ── French ────────────────────────────────────────────────────────────────────
lang_map!(Fr, "Français", [
    "app.title"          => "Simulateur QSO CW",
    "app.quit"           => "Appuyez sur Q pour quitter",
    "menu.wpm"           => "Vitesse (WPM)",
    "menu.tone"          => "Tonalité (Hz)",
    "menu.adapter"       => "Adaptateur manipulateur",
    "menu.whoStarts"     => "Qui commence le QSO",
    "menu.style"         => "Style QSO",
    "menu.lang"          => "Langue",
    "label.tx"           => "SIM TX",
    "label.rx"           => "VOTRE RX",
    "label.decoded"      => "Décodé",
    "label.status"       => "Statut",
    "status.waiting"     => "En attente de votre signal…",
    "status.listening"   => "Écoute…",
    "status.transmitting"=> "Émission…",
    "status.qso_end"     => "QSO terminé  —  73 de SIM",
    "err.no_port"        => "Aucun port série trouvé. Utilisez --port.",
    "whoStarts.me"       => "Je commence (envoyer CQ)",
    "whoStarts.sim"      => "Le simulateur commence (envoie CQ)",
    "style.ragchew"      => "Bavardage",
    "style.contest"      => "Concours",
    "style.dx"           => "DX pile-up",
    "style.random"       => "Aléatoire",
]);

// ── Italian ───────────────────────────────────────────────────────────────────
lang_map!(It, "Italiano", [
    "app.title"          => "Simulatore QSO CW",
    "app.quit"           => "Premi Q per uscire",
    "menu.wpm"           => "Velocità (WPM)",
    "menu.tone"          => "Tono (Hz)",
    "menu.adapter"       => "Adattatore manipolatore",
    "menu.whoStarts"     => "Chi inizia il QSO",
    "menu.style"         => "Stile QSO",
    "menu.lang"          => "Lingua",
    "label.tx"           => "SIM TX",
    "label.rx"           => "TUO RX",
    "label.decoded"      => "Decodificato",
    "label.status"       => "Stato",
    "status.waiting"     => "In attesa del tuo segnale…",
    "status.listening"   => "In ascolto…",
    "status.transmitting"=> "Trasmissione…",
    "status.qso_end"     => "QSO terminato  —  73 de SIM",
    "err.no_port"        => "Nessuna porta seriale trovata. Usa --port.",
    "whoStarts.me"       => "Inizio io (invio CQ)",
    "whoStarts.sim"      => "Il simulatore inizia (invia CQ)",
    "style.ragchew"      => "Chiacchierata",
    "style.contest"      => "Contest",
    "style.dx"           => "DX pile-up",
    "style.random"       => "Casuale",
]);
