// src/tui/mod.rs  —  ratatui terminal interface
#[cfg(feature = "tui")]
mod inner;
#[cfg(feature = "tui")]
pub use inner::Tui;

#[cfg(not(feature = "tui"))]
pub struct Tui;
#[cfg(not(feature = "tui"))]
impl Tui {
    pub fn new(_lang: &str) -> anyhow::Result<Self> { Ok(Self) }
    pub fn draw(&mut self, _state: &crate::AppState) -> anyhow::Result<()> { Ok(()) }
    pub fn cleanup(&mut self) {}
}
