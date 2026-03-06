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
    // ── CLI help strings ──────────────────────────────────────────────────────
    "cli.about"              => "Morse Code QSO Simulator  |  DD6DS",
    "cli.usage"              => "Usage:",
    "cli.options"            => "Options:",
    "cli.help.config"        => "Config file path (default: ~/.config/cw-qso-sim/config.toml)",
    "cli.help.mycall"        => "Your callsign (e.g. DD6DS)",
    "cli.help.sim_wpm"       => "Simulator TX speed in WPM (default: 25)",
    "cli.help.user_wpm"      => "Your keying speed in WPM — controls decoder timing (default: 18)",
    "cli.help.farnsworth"    => "Farnsworth effective WPM — stretches inter-character gaps; 0 = off (default: 0)",
    "cli.help.tone"          => "Sidetone frequency in Hz",
    "cli.help.who_starts"    => "Who starts the QSO: me | sim",
    "cli.help.style"         => "QSO style: ragchew | contest | dx-pileup | darc-cw-contest | mwc-contest | cwt-contest | wwa-contest | wpx-contest | qtt-award | sst-contest | cq-dx | pota | sota | tota | cota | random",
    "cli.help.cwt_name"      => "Your operator name for CWT contest exchange (e.g. HANS)",
    "cli.help.cwt_nr"        => "Your CWT member number or state/country (e.g. 1234 or DL)",
    "cli.help.my_dok"        => "Your DARC DOK for darc-cw-contest (e.g. P53). Use NM if not a DARC member.",
    "cli.help.adapter"       => "Keyer adapter: auto | vband | attiny85 | arduino-nano | arduino-uno | esp32 | esp8266 | winkeyer | keyboard",
    "cli.help.port"          => "Serial port for arduino-nano, arduino-uno, esp32, esp8266 or winkeyer (e.g. /dev/ttyUSB0, COM3)",
    "cli.help.midi_port"     => "MIDI port name or substring for ATtiny85 adapter (overrides --port)",
    "cli.help.paddle_mode"   => "Paddle mode: iambic_a | iambic_b | straight",
    "cli.help.switch_paddle" => "Swap DIT and DAH paddles",
    "cli.help.lang"          => "UI language: en | de | fr | it",
    "cli.help.list_ports"    => "List available HID/serial keyer devices and exit",
    "cli.help.check_adapter" => "Test the configured adapter: press DIT then DAH when prompted",
    "cli.help.write_config"  => "Write the built-in default config.toml to the config path and exit",
    "cli.help.print_config"  => "Print the built-in default config.toml to stdout and exit",
    "cli.help.demo"          => "Demo mode: play a complete QSO automatically (no keyer needed), then wait for ESC to exit",
    "cli.help.no_decode"     => "Hide decoded CW text on screen — QSO still advances; useful for self-testing without a cheat-sheet",
    "cli.help.version"       => "Print version",
    "cli.help.help"          => "Print help",
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
    // ── CLI-Hilfe ─────────────────────────────────────────────────────────────
    "cli.about"              => "Morsecode-QSO-Simulator  |  DD6DS",
    "cli.usage"              => "Verwendung:",
    "cli.options"            => "Optionen:",
    "cli.help.config"        => "Konfigurationsdatei (Standard: ~/.config/cw-qso-sim/config.toml)",
    "cli.help.mycall"        => "Dein Rufzeichen (z.B. DD6DS)",
    "cli.help.sim_wpm"       => "Simulator-Sendegeschwindigkeit in WPM (Standard: 25)",
    "cli.help.user_wpm"      => "Deine Gebegeschwindigkeit in WPM — steuert den Decoder (Standard: 18)",
    "cli.help.farnsworth"    => "Farnsworth-WPM — streckt Zeichenzwischenräume; 0 = deaktiviert (Standard: 0)",
    "cli.help.tone"          => "Mithörton-Frequenz in Hz",
    "cli.help.who_starts"    => "Wer beginnt das QSO: me | sim",
    "cli.help.style"         => "QSO-Stil: ragchew | contest | dx-pileup | darc-cw-contest | mwc-contest | cwt-contest | wwa-contest | wpx-contest | qtt-award | sst-contest | cq-dx | pota | sota | tota | cota | random",
    "cli.help.cwt_name"      => "Dein Rufname für den CWT-Contest-Austausch (z.B. HANS)",
    "cli.help.cwt_nr"        => "Deine CWT-Mitgliedsnummer oder DXCC-Kürzel (z.B. 1234 oder DL)",
    "cli.help.my_dok"        => "Dein DARC-DOK für darc-cw-contest (z.B. P53). NM wenn kein DARC-Mitglied.",
    "cli.help.adapter"       => "Keyer-Adapter: auto | vband | attiny85 | arduino-nano | arduino-uno | esp32 | esp8266 | winkeyer | keyboard",
    "cli.help.port"          => "Serieller Port für arduino-nano, arduino-uno, esp32, esp8266 oder winkeyer (z.B. /dev/ttyUSB0, COM3)",
    "cli.help.midi_port"     => "MIDI-Portname oder -Teil für den ATtiny85-Adapter (überschreibt --port)",
    "cli.help.paddle_mode"   => "Paddle-Modus: iambic_a | iambic_b | straight",
    "cli.help.switch_paddle" => "DIT- und DAH-Paddle vertauschen",
    "cli.help.lang"          => "Sprache der Benutzeroberfläche: en | de | fr | it",
    "cli.help.list_ports"    => "Verfügbare HID/Seriell-Keyer-Geräte auflisten und beenden",
    "cli.help.check_adapter" => "Konfigurierten Adapter testen: DIT dann DAH drücken wenn aufgefordert",
    "cli.help.write_config"  => "Standard-config.toml in den Konfigurationspfad schreiben und beenden",
    "cli.help.print_config"  => "Eingebaute Standard-config.toml auf stdout ausgeben und beenden",
    "cli.help.demo"          => "Demo-Modus: vollständiges QSO automatisch spielen (kein Keyer nötig), dann auf ESC warten",
    "cli.help.no_decode"     => "CW-Dekodierung ausblenden — QSO läuft weiter; nützlich zum Selbsttest ohne Spickzettel",
    "cli.help.version"       => "Version anzeigen",
    "cli.help.help"          => "Hilfe anzeigen",
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
    // ── Aide CLI ──────────────────────────────────────────────────────────────
    "cli.about"              => "Simulateur QSO CW  |  DD6DS",
    "cli.usage"              => "Utilisation :",
    "cli.options"            => "Options :",
    "cli.help.config"        => "Chemin du fichier de configuration (défaut : ~/.config/cw-qso-sim/config.toml)",
    "cli.help.mycall"        => "Votre indicatif (ex. DD6DS)",
    "cli.help.sim_wpm"       => "Vitesse d'émission du simulateur en MPM (défaut : 25)",
    "cli.help.user_wpm"      => "Votre vitesse de manipulation en MPM — contrôle le décodeur (défaut : 18)",
    "cli.help.farnsworth"    => "MPM Farnsworth effectif — étire les espaces entre caractères ; 0 = désactivé (défaut : 0)",
    "cli.help.tone"          => "Fréquence de l'écoute de contrôle en Hz",
    "cli.help.who_starts"    => "Qui commence le QSO : me | sim",
    "cli.help.style"         => "Style du QSO : ragchew | contest | dx-pileup | darc-cw-contest | mwc-contest | cwt-contest | wwa-contest | wpx-contest | qtt-award | sst-contest | cq-dx | pota | sota | tota | cota | random",
    "cli.help.cwt_name"      => "Votre nom d'opérateur pour l'échange CWT (ex. HANS)",
    "cli.help.cwt_nr"        => "Votre numéro de membre CWT ou pays/état (ex. 1234 ou DL)",
    "cli.help.my_dok"        => "Votre DOK DARC pour darc-cw-contest (ex. P53). Utilisez NM si non-membre.",
    "cli.help.adapter"       => "Adaptateur manipulateur : auto | vband | attiny85 | arduino-nano | arduino-uno | esp32 | esp8266 | winkeyer | keyboard",
    "cli.help.port"          => "Port série pour arduino-nano, arduino-uno, esp32, esp8266 ou winkeyer (ex. /dev/ttyUSB0, COM3)",
    "cli.help.midi_port"     => "Nom ou fragment du port MIDI pour l'adaptateur ATtiny85 (remplace --port)",
    "cli.help.paddle_mode"   => "Mode du manipulateur : iambic_a | iambic_b | straight",
    "cli.help.switch_paddle" => "Intervertir les palettes DIT et DAH",
    "cli.help.lang"          => "Langue de l'interface : en | de | fr | it",
    "cli.help.list_ports"    => "Lister les appareils HID/série disponibles et quitter",
    "cli.help.check_adapter" => "Tester l'adaptateur configuré : appuyer sur DIT puis DAH quand demandé",
    "cli.help.write_config"  => "Écrire le config.toml par défaut dans le chemin de configuration et quitter",
    "cli.help.print_config"  => "Afficher le config.toml intégré sur stdout et quitter",
    "cli.help.demo"          => "Mode démo : jouer un QSO complet automatiquement (pas de manipulateur requis), puis attendre ESC",
    "cli.help.no_decode"     => "Masquer le décodage CW à l'écran — le QSO avance quand même ; utile pour s'auto-évaluer sans aide",
    "cli.help.version"       => "Afficher la version",
    "cli.help.help"          => "Afficher l'aide",
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
    // ── Aiuto CLI ─────────────────────────────────────────────────────────────
    "cli.about"              => "Simulatore QSO CW  |  DD6DS",
    "cli.usage"              => "Utilizzo:",
    "cli.options"            => "Opzioni:",
    "cli.help.config"        => "Percorso del file di configurazione (predefinito: ~/.config/cw-qso-sim/config.toml)",
    "cli.help.mycall"        => "Il tuo nominativo (es. DD6DS)",
    "cli.help.sim_wpm"       => "Velocità di trasmissione del simulatore in WPM (predefinito: 25)",
    "cli.help.user_wpm"      => "La tua velocità di manipolazione in WPM — controlla il decoder (predefinito: 18)",
    "cli.help.farnsworth"    => "WPM Farnsworth effettivo — allunga gli spazi tra caratteri; 0 = disattivato (predefinito: 0)",
    "cli.help.tone"          => "Frequenza del tono di ascolto in Hz",
    "cli.help.who_starts"    => "Chi inizia il QSO: me | sim",
    "cli.help.style"         => "Stile QSO: ragchew | contest | dx-pileup | darc-cw-contest | mwc-contest | cwt-contest | wwa-contest | wpx-contest | qtt-award | sst-contest | cq-dx | pota | sota | tota | cota | random",
    "cli.help.cwt_name"      => "Il tuo nome operatore per lo scambio CWT (es. HANS)",
    "cli.help.cwt_nr"        => "Il tuo numero di membro CWT o stato/paese (es. 1234 o DL)",
    "cli.help.my_dok"        => "Il tuo DOK DARC per darc-cw-contest (es. P53). Usa NM se non sei membro.",
    "cli.help.adapter"       => "Adattatore manipolatore: auto | vband | attiny85 | arduino-nano | arduino-uno | esp32 | esp8266 | winkeyer | keyboard",
    "cli.help.port"          => "Porta seriale per arduino-nano, arduino-uno, esp32, esp8266 o winkeyer (es. /dev/ttyUSB0, COM3)",
    "cli.help.midi_port"     => "Nome o frammento della porta MIDI per l'adattatore ATtiny85 (sovrascrive --port)",
    "cli.help.paddle_mode"   => "Modalità paddle: iambic_a | iambic_b | straight",
    "cli.help.switch_paddle" => "Scambia i paddle DIT e DAH",
    "cli.help.lang"          => "Lingua dell'interfaccia: en | de | fr | it",
    "cli.help.list_ports"    => "Elenca i dispositivi HID/seriali disponibili ed esci",
    "cli.help.check_adapter" => "Testa l'adattatore configurato: premi DIT poi DAH quando richiesto",
    "cli.help.write_config"  => "Scrivi il config.toml predefinito nel percorso di configurazione ed esci",
    "cli.help.print_config"  => "Stampa il config.toml integrato su stdout ed esci",
    "cli.help.demo"          => "Modalità demo: esegui un QSO completo automaticamente (nessun manipolatore necessario), poi attendi ESC",
    "cli.help.no_decode"     => "Nasconde la decodifica CW a schermo — il QSO avanza normalmente; utile per auto-valutarsi senza supporto",
    "cli.help.version"       => "Mostra la versione",
    "cli.help.help"          => "Mostra l'aiuto",
]);
