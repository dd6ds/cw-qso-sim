// src/qso/callsigns.rs  —  Large embedded callsign + name/QTH pool
use rand::seq::SliceRandom;

pub struct SimStation {
    pub call:    &'static str,
    pub name:    &'static str,
    pub qth:     &'static str,
    pub country: &'static str,
    pub dok:     &'static str,   // DARC DOK, or "NM" for non-members
    pub cwt_ex:  &'static str,   // CWT exchange: 4-digit member nr OR state/country for non-members
    pub spc:     &'static str,   // SST SPC: US/VE/VK state or province; DXCC prefix for others
}

pub static STATIONS: &[SimStation] = &[
    SimStation { call:"DL1ABC", name:"HANS",    qth:"BERLIN",    country:"DL",  dok:"D01", cwt_ex:"1812", spc:"DL"  },
    SimStation { call:"DL2XYZ", name:"PETER",   qth:"HAMBURG",   country:"DL",  dok:"H09", cwt_ex:"DL",   spc:"DL"  },
    SimStation { call:"DL5QRS", name:"FRITZ",   qth:"MUNICH",    country:"DL",  dok:"M02", cwt_ex:"3047", spc:"DL"  },
    SimStation { call:"OE3KAB", name:"WALTER",  qth:"VIENNA",    country:"OE",  dok:"NM",  cwt_ex:"OE",   spc:"OE"  },
    SimStation { call:"PA3ABC", name:"JAN",     qth:"AMSTERDAM", country:"PA",  dok:"NM",  cwt_ex:"1563", spc:"PA"  },
    SimStation { call:"G4XYZ",  name:"JOHN",    qth:"LONDON",    country:"G",   dok:"NM",  cwt_ex:"G",    spc:"G"   },
    SimStation { call:"ON4ABC", name:"LUC",     qth:"BRUSSELS",  country:"ON",  dok:"NM",  cwt_ex:"ON",   spc:"ON"  },
    SimStation { call:"F5NTX",  name:"PIERRE",  qth:"PARIS",     country:"F",   dok:"NM",  cwt_ex:"2291", spc:"F"   },
    SimStation { call:"I2ABC",  name:"MARCO",   qth:"MILAN",     country:"I",   dok:"NM",  cwt_ex:"I",    spc:"I"   },
    SimStation { call:"SM5XY",  name:"LARS",    qth:"STOCKHOLM", country:"SM",  dok:"NM",  cwt_ex:"SM",   spc:"SM"  },
    SimStation { call:"SP5ZAP", name:"TOMASZ",  qth:"WARSAW",    country:"SP",  dok:"NM",  cwt_ex:"SP",   spc:"SP"  },
    SimStation { call:"UT5UDX", name:"SERGIY",  qth:"KYIV",      country:"UT",  dok:"NM",  cwt_ex:"UT",   spc:"UT"  },
    SimStation { call:"UA9XYZ", name:"IVAN",    qth:"MOSCOW",    country:"UA",  dok:"NM",  cwt_ex:"UA",   spc:"UA"  },
    SimStation { call:"W1AW",   name:"HIRAM",   qth:"NEWINGTON", country:"W",   dok:"NM",  cwt_ex:"CT",   spc:"CT"  },
    SimStation { call:"K5ZD",   name:"RANDY",   qth:"HARVARD",   country:"W",   dok:"NM",  cwt_ex:"MA",   spc:"MA"  },
    SimStation { call:"VE3XYZ", name:"MIKE",    qth:"TORONTO",   country:"VE",  dok:"NM",  cwt_ex:"ON",   spc:"ON"  },
    SimStation { call:"JA1ABC", name:"KENJI",   qth:"TOKYO",     country:"JA",  dok:"NM",  cwt_ex:"JA",   spc:"JA"  },
    SimStation { call:"VK2XYZ", name:"BRUCE",   qth:"SYDNEY",    country:"VK",  dok:"NM",  cwt_ex:"VK",   spc:"VK"  },
    SimStation { call:"ZL2ABC", name:"NEIL",    qth:"AUCKLAND",  country:"ZL",  dok:"NM",  cwt_ex:"ZL",   spc:"ZL"  },
    SimStation { call:"HB9ABC", name:"BEAT",    qth:"ZURICH",    country:"HB9", dok:"NM",  cwt_ex:"HB",   spc:"HB"  },
    SimStation { call:"OK2XYZ", name:"JIRI",    qth:"BRNO",      country:"OK",  dok:"NM",  cwt_ex:"OK",   spc:"OK"  },
    SimStation { call:"YL3ABC", name:"JANIS",   qth:"RIGA",      country:"YL",  dok:"NM",  cwt_ex:"YL",   spc:"YL"  },
    SimStation { call:"LY5T",   name:"TOMAS",   qth:"VILNIUS",   country:"LY",  dok:"NM",  cwt_ex:"LY",   spc:"LY"  },
    SimStation { call:"ES5TV",  name:"TONNO",   qth:"TALLINN",   country:"ES",  dok:"NM",  cwt_ex:"ES",   spc:"ES"  },
    SimStation { call:"OH2BH",  name:"MARTTI",  qth:"HELSINKI",  country:"OH",  dok:"NM",  cwt_ex:"OH",   spc:"OH"  },
    SimStation { call:"LA5YJ",  name:"BJORN",   qth:"OSLO",      country:"LA",  dok:"NM",  cwt_ex:"LA",   spc:"LA"  },
    SimStation { call:"OZ5E",   name:"FLEMMING",qth:"COPENHAGEN",country:"OZ",  dok:"NM",  cwt_ex:"OZ",   spc:"OZ"  },
    SimStation { call:"EI5DI",  name:"SEAN",    qth:"DUBLIN",    country:"EI",  dok:"NM",  cwt_ex:"EI",   spc:"EI"  },
    SimStation { call:"GM4ZUK", name:"ANGUS",   qth:"EDINBURGH", country:"GM",  dok:"NM",  cwt_ex:"GM",   spc:"GM"  },
    SimStation { call:"TF3CW",  name:"SIGGI",   qth:"REYKJAVIK", country:"TF",  dok:"NM",  cwt_ex:"TF",   spc:"TF"  },
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

/// Pick only from German (DL) stations — used for DARC CW contest so the
/// SIM always has a valid DOK instead of "NM".
pub fn random_dl_station<R: rand::Rng>(rng: &mut R) -> &'static SimStation {
    let dl: Vec<&'static SimStation> = STATIONS.iter()
        .filter(|s| s.country == "DL")
        .collect();
    dl.choose(rng).copied().unwrap_or_else(|| STATIONS.choose(rng).unwrap())
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

/// All valid DARC DOK codes (1192 entries from the official DOK-Liste)
pub static DOK_CODES: &[&str] = &[
    "A01", "A02", "A03", "A04", "A05", "A06", "A07", "A08", "A09", "A10", "A11", "A12",
    "A13", "A14", "A15", "A16", "A17", "A18", "A19", "A20", "A21", "A22", "A23", "A24",
    "A25", "A26", "A27", "A28", "A29", "A30", "A31", "A32", "A33", "A34", "A35", "A36",
    "A37", "A38", "A39", "A40", "A41", "A42", "A43", "A44", "A45", "A46", "A47", "A48",
    "A49", "A50", "A51", "A52", "A53", "A55", "B01", "B02", "B03", "B04", "B05", "B06",
    "B07", "B08", "B09", "B10", "B11", "B12", "B13", "B14", "B15", "B16", "B17", "B18",
    "B19", "B20", "B21", "B22", "B23", "B24", "B25", "B26", "B27", "B28", "B29", "B30",
    "B31", "B32", "B33", "B34", "B35", "B36", "B37", "B38", "B39", "B40", "B41", "B42",
    "B43", "C01", "C02", "C03", "C04", "C05", "C06", "C07", "C08", "C09", "C10", "C11",
    "C12", "C13", "C14", "C15", "C16", "C17", "C18", "C19", "C20", "C21", "C22", "C23",
    "C24", "C25", "C26", "C27", "C28", "C29", "C30", "C31", "C32", "C33", "C34", "C35",
    "C36", "C37", "C73", "D01", "D02", "D03", "D04", "D05", "D06", "D07", "D08", "D09",
    "D10", "D11", "D12", "D13", "D14", "D15", "D16", "D17", "D18", "D19", "D20", "D21",
    "D22", "D23", "D24", "D25", "D26", "D27", "D28", "E01", "E02", "E03", "E04", "E05",
    "E06", "E07", "E08", "E09", "E10", "E11", "E12", "E13", "E14", "E15", "E16", "E17",
    "E18", "E19", "E20", "E21", "E22", "E23", "E24", "E25", "E26", "E27", "E28", "E29",
    "E30", "E31", "E32", "E33", "E34", "E35", "E36", "E37", "E38", "E39", "F01", "F02",
    "F03", "F04", "F05", "F06", "F07", "F08", "F09", "F10", "F11", "F12", "F13", "F14",
    "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24", "F25", "F26",
    "F27", "F28", "F29", "F30", "F31", "F32", "F33", "F34", "F35", "F36", "F37", "F38",
    "F39", "F40", "F41", "F42", "F43", "F44", "F45", "F46", "F47", "F48", "F49", "F50",
    "F51", "F52", "F53", "F54", "F55", "F56", "F57", "F58", "F59", "F60", "F61", "F62",
    "F63", "F64", "F65", "F66", "F67", "F68", "F69", "F70", "F71", "F72", "F73", "F74",
    "F75", "F76", "G01", "G02", "G03", "G04", "G05", "G06", "G07", "G08", "G09", "G10",
    "G11", "G12", "G13", "G14", "G15", "G16", "G17", "G18", "G19", "G20", "G21", "G22",
    "G23", "G24", "G25", "G26", "G27", "G28", "G29", "G30", "G31", "G32", "G33", "G34",
    "G35", "G36", "G37", "G38", "G39", "G40", "G41", "G42", "G43", "G44", "G45", "G46",
    "G47", "G48", "G49", "G50", "G51", "G52", "G53", "G54", "G55", "G56", "G73", "H01",
    "H02", "H03", "H04", "H05", "H06", "H07", "H08", "H09", "H10", "H11", "H12", "H13",
    "H14", "H15", "H16", "H17", "H18", "H19", "H20", "H21", "H22", "H23", "H24", "H25",
    "H26", "H27", "H28", "H29", "H30", "H31", "H32", "H33", "H34", "H35", "H36", "H37",
    "H38", "H39", "H40", "H41", "H42", "H43", "H44", "H45", "H46", "H47", "H48", "H49",
    "H50", "H51", "H52", "H53", "H54", "H55", "H56", "H57", "H58", "H59", "H60", "H61",
    "H62", "H63", "H64", "H65", "H66", "I01", "I02", "I03", "I04", "I05", "I06", "I07",
    "I08", "I09", "I10", "I11", "I12", "I13", "I14", "I15", "I16", "I17", "I18", "I19",
    "I20", "I21", "I22", "I23", "I24", "I25", "I26", "I27", "I28", "I29", "I30", "I31",
    "I32", "I33", "I34", "I35", "I36", "I37", "I38", "I39", "I40", "I41", "I42", "I43",
    "I44", "I45", "I46", "I47", "I48", "I49", "I50", "I51", "I52", "I53", "I54", "I55",
    "I56", "I57", "I58", "K01", "K02", "K03", "K04", "K05", "K06", "K07", "K08", "K09",
    "K10", "K11", "K12", "K13", "K14", "K15", "K16", "K17", "K18", "K19", "K20", "K21",
    "K22", "K23", "K24", "K25", "K26", "K27", "K28", "K29", "K30", "K31", "K32", "K33",
    "K34", "K35", "K36", "K37", "K38", "K39", "K40", "K41", "K42", "K43", "K44", "K45",
    "K46", "K47", "K48", "K49", "K50", "K51", "K52", "K53", "K54", "K55", "K56", "K57",
    "L01", "L02", "L03", "L04", "L05", "L06", "L07", "L08", "L09", "L10", "L11", "L12",
    "L13", "L14", "L15", "L16", "L17", "L18", "L19", "L20", "L21", "L22", "L23", "L24",
    "L25", "L26", "L27", "L28", "L29", "L30", "L31", "M01", "M02", "M03", "M04", "M05",
    "M06", "M07", "M08", "M09", "M10", "M11", "M12", "M13", "M14", "M15", "M16", "M17",
    "M18", "M19", "M20", "M21", "M22", "M23", "M24", "M25", "M26", "M27", "M28", "M29",
    "M30", "M31", "M32", "M33", "M34", "M35", "M36", "N01", "N02", "N03", "N04", "N05",
    "N06", "N07", "N08", "N09", "N10", "N11", "N12", "N13", "N14", "N15", "N16", "N17",
    "N18", "N19", "N20", "N21", "N22", "N23", "N24", "N25", "N26", "N27", "N28", "N29",
    "N30", "N31", "N32", "N33", "N34", "N35", "N36", "N37", "N38", "N39", "N40", "N41",
    "N42", "N43", "N44", "N45", "N46", "N47", "N48", "N49", "N50", "N51", "N52", "N53",
    "N54", "N55", "N56", "N57", "N58", "N59", "N60", "N61", "N62", "O01", "O02", "O03",
    "O04", "O05", "O06", "O07", "O08", "O09", "O10", "O11", "O12", "O13", "O14", "O15",
    "O16", "O17", "O18", "O19", "O20", "O21", "O22", "O23", "O24", "O25", "O26", "O27",
    "O28", "O29", "O30", "O31", "O32", "O33", "O34", "O35", "O36", "O37", "O38", "O39",
    "O40", "O41", "O42", "O43", "O44", "O45", "O46", "O47", "O48", "O49", "O50", "O51",
    "O52", "O53", "O54", "O55", "P01", "P02", "P03", "P04", "P05", "P06", "P07", "P08",
    "P09", "P10", "P11", "P12", "P13", "P14", "P15", "P16", "P17", "P18", "P19", "P20",
    "P21", "P22", "P23", "P24", "P25", "P26", "P27", "P28", "P29", "P30", "P31", "P32",
    "P33", "P34", "P35", "P36", "P37", "P38", "P39", "P40", "P41", "P42", "P43", "P44",
    "P45", "P46", "P47", "P48", "P49", "P50", "P51", "P52", "P53", "P54", "P55", "P56",
    "P57", "P58", "P59", "P60", "P61", "P62", "Q01", "Q02", "Q03", "Q04", "Q05", "Q06",
    "Q07", "Q08", "Q09", "Q10", "Q11", "Q12", "Q13", "Q14", "Q15", "Q16", "Q17", "Q18",
    "Q19", "Q20", "Q21", "R01", "R02", "R03", "R04", "R05", "R06", "R07", "R08", "R09",
    "R10", "R11", "R12", "R13", "R14", "R15", "R16", "R17", "R18", "R19", "R20", "R21",
    "R22", "R23", "R24", "R25", "R26", "R27", "R28", "R29", "R30", "R31", "R32", "R33",
    "R34", "R55", "S01", "S02", "S03", "S04", "S05", "S06", "S07", "S08", "S09", "S10",
    "S11", "S12", "S13", "S14", "S15", "S16", "S17", "S18", "S19", "S20", "S21", "S22",
    "S23", "S24", "S25", "S26", "S27", "S28", "S29", "S30", "S31", "S32", "S33", "S34",
    "S35", "S36", "S37", "S38", "S39", "S40", "S41", "S42", "S43", "S44", "S45", "S46",
    "S47", "S48", "S49", "S50", "S51", "S52", "S53", "S54", "S55", "S56", "S57", "S58",
    "S59", "S60", "S61", "S62", "S63", "S64", "S65", "S66", "S67", "S68", "S69", "S70",
    "T01", "T02", "T03", "T04", "T05", "T06", "T07", "T08", "T09", "T10", "T11", "T12",
    "T13", "T14", "T15", "T16", "T17", "T18", "T19", "T20", "T21", "U01", "U02", "U03",
    "U04", "U05", "U06", "U07", "U08", "U09", "U10", "U11", "U12", "U13", "U14", "U15",
    "U16", "U17", "U18", "U19", "U20", "U21", "U22", "U23", "U24", "U25", "U26", "U27",
    "U28", "U29", "U30", "V01", "V02", "V03", "V04", "V05", "V06", "V07", "V08", "V09",
    "V10", "V11", "V12", "V13", "V14", "V15", "V16", "V17", "V18", "V19", "V20", "V21",
    "V22", "V23", "V24", "V25", "V26", "V27", "V28", "V29", "V30", "W01", "W02", "W03",
    "W04", "W05", "W06", "W07", "W08", "W09", "W10", "W11", "W12", "W13", "W14", "W15",
    "W16", "W17", "W18", "W19", "W20", "W21", "W22", "W23", "W24", "W25", "W26", "W27",
    "W28", "W29", "W30", "W31", "W32", "W33", "W34", "W35", "W36", "W37", "W38", "X01",
    "X02", "X03", "X04", "X05", "X06", "X07", "X08", "X09", "X10", "X11", "X12", "X13",
    "X14", "X15", "X16", "X17", "X18", "X19", "X20", "X21", "X22", "X23", "X24", "X25",
    "X26", "X27", "X28", "X29", "X30", "X31", "X32", "X33", "X34", "X35", "X36", "X37",
    "X38", "X39", "X40", "X41", "X42", "X43", "X44", "X45", "X46", "X47", "X48", "Y01",
    "Y02", "Y03", "Y04", "Y05", "Y06", "Y07", "Y08", "Y09", "Y10", "Y11", "Y12", "Y13",
    "Y14", "Y15", "Y16", "Y17", "Y18", "Y19", "Y20", "Y21", "Y22", "Y23", "Y24", "Y25",
    "Y26", "Y27", "Y28", "Y29", "Y30", "Y31", "Y32", "Y33", "Y34", "Y35", "Y36", "Y37",
    "Y38", "Y39", "Y40", "Y41", "Y42", "Y43", "Z01", "Z02", "Z03", "Z04", "Z05", "Z06",
    "Z07", "Z08", "Z09", "Z10", "Z11", "Z12", "Z13", "Z14", "Z15", "Z16", "Z17", "Z18",
    "Z19", "Z20", "Z21", "Z22", "Z23", "Z24", "Z25", "Z26", "Z27", "Z28", "Z29", "Z30",
    "Z31", "Z32", "Z33", "Z34", "Z35", "Z36", "Z37", "Z38", "Z39", "Z40", "Z41", "Z42",
    "Z43", "Z44", "Z45", "Z46", "Z47", "Z48", "Z49", "Z50", "Z51", "Z52", "Z53", "Z54",
    "Z55", "Z56", "Z57", "Z58", "Z59", "Z60", "Z61", "Z62", "Z63", "Z64", "Z65", "Z66",
    "Z67", "Z68", "Z69", "Z70", "Z71", "Z72", "Z73", "Z74", "Z75", "Z76", "Z77", "Z78",
    "Z79", "Z80", "Z81", "Z82", "Z83", "Z84", "Z85", "Z86", "Z87", "Z88", "Z89", "Z90",
    "Z91", "Z92", "Z93", "Z94",
];

/// Pick a random DOK code from the official pool
pub fn random_dok<R: rand::Rng>(rng: &mut R) -> &'static str {
    DOK_CODES.choose(rng).unwrap()
}

/// Official WWA (World Wide Award) special station callsigns — sourced from
/// https://hamaward.cloud/wwa/teams  (2026 edition, 118 entries)
pub static WWA_CALLSIGNS: &[&str] = &[
    "3B8WWA", "3Z6I",    "4M5A",    "4M5DX",   "4U1A",    "5B4WWA",  "8A1A",
    "9M2WWA", "9M8WWA",  "A43WWA",  "A65D",    "AT2WWA",  "AT3WWA",  "AT4WWA",
    "AT6WWA", "AT7WWA",  "BA3RA",   "BA7CK",   "BG0DXC",  "BH9CA",   "BI4SSB",
    "BY1RX",  "BY2WL",   "BY5HB",   "BY6SX",   "BY8MA",   "CQ7WWA",  "CR2WWA",
    "CR5WWA", "CR6WWA",  "D4W",     "DA0WWA",  "DL0WWA",  "DU0WWA",  "E2WWA",
    "E7W",    "EG1WWA",  "EG2WWA",  "EG3WWA",  "EG4WWA",  "EG5WWA",  "EG6WWA",
    "EG7WWA", "EG8WW",   "EG9WWA",  "EM0WWA",  "GB0WWA",  "GB1WWA",  "GB2WWA",
    "GB4WWA", "GB5WWA",  "GB6WWA",  "GB8WWA",  "GB9WWA",  "HB9WWA",  "HI3WWA",
    "HI6WWA", "HI7WWA",  "HI8WWA",  "HZ1WWA",  "II0WWA",  "II1WWA",  "II2WWA",
    "II3WWA", "II4WWA",  "II5WWA",  "II6WWA",  "II7WWA",  "II8WWA",  "II9WWA",
    "IR0WWA", "IR1WWA",  "LA1WWA",  "LR1WWA",  "LZ0WWA",  "N0W",     "N1W",
    "N4W",    "N6W",     "N8W",     "N9W",     "OL6WWA",  "OP0WWA",  "PA26WWA",
    "PC26WWA","PD26WWA", "PE26WWA", "PF26WWA", "RU0LL",   "RW1F",    "S53WWA",
    "SB9WWA", "SC9WWA",  "SD9WWA",  "SN0WWA",  "SN1WWA",  "SN2WWA",  "SN3WWA",
    "SN4WWA", "SN6WWA",  "SO3WWA",  "SX0W",    "TK4TH",   "TM18WWA", "TM1WWA",
    "TM29WWA","TM7WWA",  "TM9WWA",  "UP7WWA",  "VB2WWA",  "VC1WWA",  "VE9WWA",
    "VJ6X",   "VR2WAA",  "W4I",     "YI1RN",   "YL73R",   "YO0WWA",  "YU45MJA",
    "Z30WWA", "ZW5B",
];

/// Pick a random WWA special station callsign
pub fn random_wwa_callsign<R: rand::Rng>(rng: &mut R) -> &'static str {
    WWA_CALLSIGNS.choose(rng).unwrap()
}

/// Generate a POTA (Parks on the Air) park reference based on the station's country.
/// Format: {prefix}-{NNNN}  e.g. K-1234, DL-0042, VE-0567
pub fn random_pota_ref<R: rand::Rng>(rng: &mut R, country: &str) -> String {
    let prefix = match country {
        "W"   => "K",
        "VE"  => "VE",
        "DL"  => "DL",
        "G"   => "G",
        "GM"  => "GM",
        "EI"  => "EI",
        "F"   => "F",
        "I"   => "I",
        "SM"  => "SM",
        "OH"  => "OH",
        "OE"  => "OE",
        "PA"  => "PA",
        "ON"  => "ON",
        "SP"  => "SP",
        "OK"  => "OK",
        "HB9" => "HB",
        "JA"  => "JA",
        "VK"  => "VK",
        "ZL"  => "ZL",
        "LA"  => "LA",
        "OZ"  => "OZ",
        "LY"  => "LY",
        "YL"  => "YL",
        "ES"  => "ES",
        "TF"  => "TF",
        "UT"  => "UT",
        "UA"  => "RA",
        _     => "K",
    };
    let nr = rng.gen_range(1u32..=9999);
    format!("{prefix}-{nr:04}")
}

/// Generate a SOTA (Summits on the Air) summit reference based on the station's country.
/// Format: {association}/{region}-{NNN}  e.g. DL/AL-042, W6/NC-001
pub fn random_sota_ref<R: rand::Rng>(rng: &mut R, country: &str) -> String {
    let (assoc, region) = match country {
        "W"   => ("W1",  "WR"),
        "VE"  => ("VE3", "ON"),
        "DL"  => ("DL",  "AL"),
        "G"   => ("G",   "NW"),
        "GM"  => ("GM",  "SS"),
        "EI"  => ("EI",  "IE"),
        "F"   => ("F",   "CO"),
        "I"   => ("I",   "LO"),
        "SM"  => ("SM",  "SD"),
        "OH"  => ("OH",  "JS"),
        "OE"  => ("OE",  "ST"),
        "PA"  => ("PA",  "PA"),
        "ON"  => ("ON",  "ON"),
        "HB9" => ("HB",  "AG"),
        "JA"  => ("JA",  "KG"),
        "VK"  => ("VK3", "VC"),
        "ZL"  => ("ZL3", "CB"),
        "LA"  => ("LA",  "TM"),
        "OZ"  => ("OZ",  "FYN"),
        "LY"  => ("LY",  "KA"),
        "YL"  => ("YL",  "RI"),
        "ES"  => ("ES",  "HA"),
        "TF"  => ("TF",  "SW"),
        "SP"  => ("SP",  "BZ"),
        "OK"  => ("OK",  "JM"),
        "UT"  => ("UT",  "CR"),
        _     => ("W1",  "WR"),
    };
    let nr = rng.gen_range(1u32..=999);
    format!("{assoc}/{region}-{nr:03}")
}

/// Generate a COTA (Castles on the Air) castle reference based on the station's country.
/// Format: {country_code}/CA-{NNN}  e.g. GB/CA-042, DL/CA-007
pub fn random_cota_ref<R: rand::Rng>(rng: &mut R, country: &str) -> String {
    let code = match country {
        "W"   => "US",
        "VE"  => "CA",
        "DL"  => "DL",
        "G"   | "GM" | "EI" => "GB",
        "F"   => "FR",
        "I"   => "IT",
        "SM"  => "SE",
        "OH"  => "FI",
        "OE"  => "AT",
        "PA"  => "NL",
        "ON"  => "BE",
        "SP"  => "PL",
        "OK"  => "CZ",
        "HB9" => "CH",
        "JA"  => "JP",
        "VK"  => "AU",
        "ZL"  => "NZ",
        "LA"  => "NO",
        "OZ"  => "DK",
        "LY"  => "LT",
        "YL"  => "LV",
        "ES"  => "EE",
        "TF"  => "IS",
        "UT"  => "UA",
        _     => "GB",
    };
    let nr = rng.gen_range(1u32..=999);
    format!("{code}/CA-{nr:03}")
}

/// Derive a country code from a callsign for use with the random_*_ref generators.
/// Handles the most common amateur radio callsign prefixes worldwide.
pub fn country_from_callsign(call: &str) -> &'static str {
    let c = call.to_uppercase();
    // German prefixes: DA..DP series (all 2-letter prefixes starting with D)
    if c.starts_with("DA") || c.starts_with("DB") || c.starts_with("DC")
        || c.starts_with("DD") || c.starts_with("DE") || c.starts_with("DF")
        || c.starts_with("DG") || c.starts_with("DH") || c.starts_with("DJ")
        || c.starts_with("DK") || c.starts_with("DL") || c.starts_with("DM")
        || c.starts_with("DO") || c.starts_with("DP") { "DL" }
    // Canada (VE/VA/VO/VY — must come before VK)
    else if c.starts_with("VE") || c.starts_with("VA")
         || c.starts_with("VO") || c.starts_with("VY") { "VE" }
    // Australia
    else if c.starts_with("VK") { "VK" }
    // Poland
    else if c.starts_with("SP") { "SP" }
    // Sweden
    else if c.starts_with("SM") || c.starts_with("SA")
         || c.starts_with("SE") || c.starts_with("SK") { "SM" }
    // Finland
    else if c.starts_with("OH") { "OH" }
    // Austria
    else if c.starts_with("OE") { "OE" }
    // Belgium
    else if c.starts_with("ON") { "ON" }
    // Denmark
    else if c.starts_with("OZ") { "OZ" }
    // Czech Republic
    else if c.starts_with("OK") { "OK" }
    // Netherlands
    else if c.starts_with("PA") || c.starts_with("PD")
         || c.starts_with("PE") || c.starts_with("PH") { "PA" }
    // Spain
    else if c.starts_with("EA") { "EA" }
    // Switzerland
    else if c.starts_with("HB") { "HB9" }
    // Norway
    else if c.starts_with("LA") || c.starts_with("LB") { "LA" }
    // Lithuania
    else if c.starts_with("LY") { "LY" }
    // Latvia
    else if c.starts_with("YL") { "YL" }
    // Estonia
    else if c.starts_with("ES") { "ES" }
    // Iceland
    else if c.starts_with("TF") { "TF" }
    // Ukraine
    else if c.starts_with("UT") || c.starts_with("UR") { "UT" }
    // New Zealand
    else if c.starts_with("ZL") { "ZL" }
    // Japan
    else if c.starts_with("JA") || c.starts_with("JH")
         || c.starts_with("JR") || c.starts_with("JO") { "JA" }
    // Ireland
    else if c.starts_with("EI") { "EI" }
    // Scotland (GM before single G)
    else if c.starts_with("GM") { "GM" }
    // United Kingdom
    else if c.starts_with('G') || c.starts_with('M') { "G" }
    // France
    else if c.starts_with('F') { "F" }
    // Italy
    else if c.starts_with('I') { "I" }
    // USA: A*, K*, N*, W* (must come after two-letter prefixes handled above)
    else if c.starts_with('A') || c.starts_with('K')
         || c.starts_with('N') || c.starts_with('W') { "W" }
    // Fallback — use DL so refs are always well-formed
    else { "DL" }
}

/// Generate a TOTA (Towers on the Air) tower reference based on the station's country.
/// Format: {country_code}-{NNNN}  e.g. US-0042, DL-0123  (wwtota.com style)
pub fn random_tota_ref<R: rand::Rng>(rng: &mut R, country: &str) -> String {
    let code = match country {
        "W"   => "US",
        "VE"  => "CA",
        "DL"  => "DL",
        "G"   | "GM" | "EI" => "GB",
        "F"   => "FR",
        "I"   => "IT",
        "SM"  => "SE",
        "OH"  => "FI",
        "OE"  => "AT",
        "PA"  => "NL",
        "ON"  => "BE",
        "SP"  => "PL",
        "OK"  => "CZ",
        "HB9" => "CH",
        "JA"  => "JP",
        "VK"  => "AU",
        "ZL"  => "NZ",
        "LA"  => "NO",
        "OZ"  => "DK",
        "LY"  => "LT",
        "YL"  => "LV",
        "ES"  => "EE",
        "TF"  => "IS",
        "UT"  => "UA",
        _     => "US",
    };
    let nr = rng.gen_range(1u32..=9999);
    format!("{code}-{nr:04}")
}
