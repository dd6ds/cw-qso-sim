// src/qso/callsigns.rs  —  Large embedded callsign + name/QTH pool
use rand::seq::SliceRandom;

pub struct SimStation {
    pub call:    &'static str,
    pub name:    &'static str,
    pub qth:     &'static str,
    pub country: &'static str,
    pub dok:     &'static str,   // DARC DOK, or "NM" for non-members
}

pub static STATIONS: &[SimStation] = &[
    SimStation { call:"DL1ABC", name:"HANS",    qth:"BERLIN",    country:"DL",  dok:"D01" },
    SimStation { call:"DL2XYZ", name:"PETER",   qth:"HAMBURG",   country:"DL",  dok:"H09" },
    SimStation { call:"DL5QRS", name:"FRITZ",   qth:"MUNICH",    country:"DL",  dok:"M02" },
    SimStation { call:"OE3KAB", name:"WALTER",  qth:"VIENNA",    country:"OE",  dok:"NM"  },
    SimStation { call:"PA3ABC", name:"JAN",     qth:"AMSTERDAM", country:"PA",  dok:"NM"  },
    SimStation { call:"G4XYZ",  name:"JOHN",    qth:"LONDON",    country:"G",   dok:"NM"  },
    SimStation { call:"ON4ABC", name:"LUC",     qth:"BRUSSELS",  country:"ON",  dok:"NM"  },
    SimStation { call:"F5NTX",  name:"PIERRE",  qth:"PARIS",     country:"F",   dok:"NM"  },
    SimStation { call:"I2ABC",  name:"MARCO",   qth:"MILAN",     country:"I",   dok:"NM"  },
    SimStation { call:"SM5XY",  name:"LARS",    qth:"STOCKHOLM", country:"SM",  dok:"NM"  },
    SimStation { call:"SP5ZAP", name:"TOMASZ",  qth:"WARSAW",    country:"SP",  dok:"NM"  },
    SimStation { call:"UT5UDX", name:"SERGIY",  qth:"KYIV",      country:"UT",  dok:"NM"  },
    SimStation { call:"UA9XYZ", name:"IVAN",    qth:"MOSCOW",    country:"UA",  dok:"NM"  },
    SimStation { call:"W1AW",   name:"HIRAM",   qth:"NEWINGTON", country:"W",   dok:"NM"  },
    SimStation { call:"K5ZD",   name:"RANDY",   qth:"HARVARD",   country:"W",   dok:"NM"  },
    SimStation { call:"VE3XYZ", name:"MIKE",    qth:"TORONTO",   country:"VE",  dok:"NM"  },
    SimStation { call:"JA1ABC", name:"KENJI",   qth:"TOKYO",     country:"JA",  dok:"NM"  },
    SimStation { call:"VK2XYZ", name:"BRUCE",   qth:"SYDNEY",    country:"VK",  dok:"NM"  },
    SimStation { call:"ZL2ABC", name:"NEIL",    qth:"AUCKLAND",  country:"ZL",  dok:"NM"  },
    SimStation { call:"HB9ABC", name:"BEAT",    qth:"ZURICH",    country:"HB9", dok:"NM"  },
    SimStation { call:"OK2XYZ", name:"JIRI",    qth:"BRNO",      country:"OK",  dok:"NM"  },
    SimStation { call:"YL3ABC", name:"JANIS",   qth:"RIGA",      country:"YL",  dok:"NM"  },
    SimStation { call:"LY5T",   name:"TOMAS",   qth:"VILNIUS",   country:"LY",  dok:"NM"  },
    SimStation { call:"ES5TV",  name:"TONNO",   qth:"TALLINN",   country:"ES",  dok:"NM"  },
    SimStation { call:"OH2BH",  name:"MARTTI",  qth:"HELSINKI",  country:"OH",  dok:"NM"  },
    SimStation { call:"LA5YJ",  name:"BJORN",   qth:"OSLO",      country:"LA",  dok:"NM"  },
    SimStation { call:"OZ5E",   name:"FLEMMING",qth:"COPENHAGEN",country:"OZ",  dok:"NM"  },
    SimStation { call:"EI5DI",  name:"SEAN",    qth:"DUBLIN",    country:"EI",  dok:"NM"  },
    SimStation { call:"GM4ZUK", name:"ANGUS",   qth:"EDINBURGH", country:"GM",  dok:"NM"  },
    SimStation { call:"TF3CW",  name:"SIGGI",   qth:"REYKJAVIK", country:"TF",  dok:"NM"  },
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
