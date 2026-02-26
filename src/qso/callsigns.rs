// src/qso/callsigns.rs  —  Large embedded callsign + name/QTH pool
use rand::seq::SliceRandom;

pub struct SimStation {
    pub call:    &'static str,
    pub name:    &'static str,
    pub qth:     &'static str,
    pub country: &'static str,
}

pub static STATIONS: &[SimStation] = &[
    SimStation { call:"DL1ABC", name:"HANS",    qth:"BERLIN",    country:"DL" },
    SimStation { call:"DL2XYZ", name:"PETER",   qth:"HAMBURG",   country:"DL" },
    SimStation { call:"DL5QRS", name:"FRITZ",   qth:"MUNICH",    country:"DL" },
    SimStation { call:"OE3KAB", name:"WALTER",  qth:"VIENNA",    country:"OE" },
    SimStation { call:"PA3ABC", name:"JAN",     qth:"AMSTERDAM", country:"PA" },
    SimStation { call:"G4XYZ",  name:"JOHN",    qth:"LONDON",    country:"G"  },
    SimStation { call:"ON4ABC", name:"LUC",     qth:"BRUSSELS",  country:"ON" },
    SimStation { call:"F5NTX",  name:"PIERRE",  qth:"PARIS",     country:"F"  },
    SimStation { call:"I2ABC",  name:"MARCO",   qth:"MILAN",     country:"I"  },
    SimStation { call:"SM5XY",  name:"LARS",    qth:"STOCKHOLM", country:"SM" },
    SimStation { call:"SP5ZAP", name:"TOMASZ",  qth:"WARSAW",    country:"SP" },
    SimStation { call:"UT5UDX", name:"SERGIY",  qth:"KYIV",      country:"UT" },
    SimStation { call:"UA9XYZ", name:"IVAN",    qth:"MOSCOW",    country:"UA" },
    SimStation { call:"W1AW",   name:"HIRAM",   qth:"NEWINGTON", country:"W"  },
    SimStation { call:"K5ZD",   name:"RANDY",   qth:"HARVARD",   country:"W"  },
    SimStation { call:"VE3XYZ", name:"MIKE",    qth:"TORONTO",   country:"VE" },
    SimStation { call:"JA1ABC", name:"KENJI",   qth:"TOKYO",     country:"JA" },
    SimStation { call:"VK2XYZ", name:"BRUCE",   qth:"SYDNEY",    country:"VK" },
    SimStation { call:"ZL2ABC", name:"NEIL",    qth:"AUCKLAND",  country:"ZL" },
    SimStation { call:"HB9ABC", name:"BEAT",    qth:"ZURICH",    country:"HB9"},
    SimStation { call:"OK2XYZ", name:"JIRI",    qth:"BRNO",      country:"OK" },
    SimStation { call:"YL3ABC", name:"JANIS",   qth:"RIGA",      country:"YL" },
    SimStation { call:"LY5T",   name:"TOMAS",   qth:"VILNIUS",   country:"LY" },
    SimStation { call:"ES5TV",  name:"TONNO",   qth:"TALLINN",   country:"ES" },
    SimStation { call:"OH2BH",  name:"MARTTI",  qth:"HELSINKI",  country:"OH" },
    SimStation { call:"LA5YJ",  name:"BJORN",   qth:"OSLO",      country:"LA" },
    SimStation { call:"OZ5E",   name:"FLEMMING",qth:"COPENHAGEN",country:"OZ" },
    SimStation { call:"EI5DI",  name:"SEAN",    qth:"DUBLIN",    country:"EI" },
    SimStation { call:"GM4ZUK", name:"ANGUS",   qth:"EDINBURGH", country:"GM" },
    SimStation { call:"TF3CW",  name:"SIGGI",   qth:"REYKJAVIK", country:"TF" },
];

/// RST values realistic for CW
pub static RST_VALUES: &[&str] = &[
    "559", "569", "579", "589", "599",
    "459", "469", "479", "489",
];

/// Rig descriptions
pub static RIGS: &[&str] = &[
    "IC 7300", "IC 7610", "FT 991A", "FT 857", "TS 590", "TS 890",
    "K3", "KX3", "X6100", "ELECRAFT K4",
];

/// Antenna descriptions
pub static ANTENNAS: &[&str] = &[
    "DIPOLE", "YAGI", "VERTICAL", "LOOP", "BEAM", "WINDOM", "DELTA LOOP",
];

/// Power levels
pub static POWER: &[&str] = &[
    "5W", "10W", "50W", "100W", "200W", "400W",
];

pub fn random_station<R: rand::Rng>(rng: &mut R) -> &'static SimStation {
    STATIONS.choose(rng).unwrap()
}

pub fn random_rst<R: rand::Rng>(rng: &mut R) -> &'static str {
    RST_VALUES.choose(rng).unwrap()
}

pub fn random_rig<R: rand::Rng>(rng: &mut R) -> &'static str {
    RIGS.choose(rng).unwrap()
}

pub fn random_ant<R: rand::Rng>(rng: &mut R) -> &'static str {
    ANTENNAS.choose(rng).unwrap()
}

pub fn random_pwr<R: rand::Rng>(rng: &mut R) -> &'static str {
    POWER.choose(rng).unwrap()
}
