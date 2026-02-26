// src/qso/mod.rs  —  QSO state machine + callsign list + exchange logic
pub mod callsigns;
pub mod exchanges;
pub mod state;

pub use state::{QsoEngine, QsoEvent};
