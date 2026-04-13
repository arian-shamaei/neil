use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::state::NeilState;

/// A Panel is a self-contained UI module -- a "cartridge" that plugs into
/// the blueprint console. Each panel reads from NeilState and renders itself.
pub trait Panel {
    /// Unique identifier (used in layout config)
    fn id(&self) -> &str;

    /// Display title for the panel border
    fn title(&self) -> &str;

    /// Render into the given area
    fn render(&self, area: Rect, buf: &mut Buffer, state: &NeilState);

    /// Update internal state (called each tick)
    fn update(&mut self, _state: &NeilState) {}

    /// Single-line compact summary (for narrow mode)
    fn compact(&self, _state: &NeilState) -> String {
        String::new()
    }

    /// Priority in narrow mode (0=hidden, 1=low, 2=med, 3=high)
    fn priority(&self) -> u8 { 2 }
}
