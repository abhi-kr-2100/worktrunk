//! Styled line and string types for composable terminal output
//!
//! Provides types for building complex styled output with proper width calculation.

use anstyle::Style;
use unicode_width::UnicodeWidthStr;

/// Strip ANSI escape codes (SGR and OSC) to get visual text for width calculation
fn strip_ansi_codes(text: &str) -> String {
    String::from_utf8_lossy(&strip_ansi_escapes::strip(text)).to_string()
}

/// A piece of text with an optional style
#[derive(Clone, Debug)]
pub struct StyledString {
    pub text: String,
    pub style: Option<Style>,
}

impl StyledString {
    pub fn new(text: impl Into<String>, style: Option<Style>) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    pub fn raw(text: impl Into<String>) -> Self {
        Self::new(text, None)
    }

    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Self::new(text, Some(style))
    }

    /// Returns the visual width (unicode-aware, ANSI codes stripped)
    pub fn width(&self) -> usize {
        let clean_text = strip_ansi_codes(&self.text);
        clean_text.width()
    }

    /// Renders to a string with ANSI escape codes
    pub fn render(&self) -> String {
        if let Some(style) = &self.style {
            format!("{}{}{}", style.render(), self.text, style.render_reset())
        } else {
            self.text.clone()
        }
    }
}

/// A line composed of multiple styled strings
#[derive(Clone, Debug, Default)]
pub struct StyledLine {
    pub segments: Vec<StyledString>,
}

impl StyledLine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a raw (unstyled) segment
    pub fn push_raw(&mut self, text: impl Into<String>) {
        self.segments.push(StyledString::raw(text));
    }

    /// Add a styled segment
    pub fn push_styled(&mut self, text: impl Into<String>, style: Style) {
        self.segments.push(StyledString::styled(text, style));
    }

    /// Add a segment (StyledString)
    pub fn push(&mut self, segment: StyledString) {
        self.segments.push(segment);
    }

    /// Append every segment from another styled line.
    pub fn extend(&mut self, other: StyledLine) {
        self.segments.extend(other.segments);
    }

    /// Pad with spaces to reach a specific width
    pub fn pad_to(&mut self, target_width: usize) {
        let current_width = self.width();
        if current_width < target_width {
            self.push_raw(" ".repeat(target_width - current_width));
        }
    }

    /// Returns the total visual width
    pub fn width(&self) -> usize {
        self.segments.iter().map(|s| s.width()).sum()
    }

    /// Renders the entire line with ANSI escape codes
    pub fn render(&self) -> String {
        self.segments.iter().map(|s| s.render()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_width_strips_osc_hyperlinks() {
        // Text with OSC 8 hyperlink should have visual width of just the text
        let url = "https://github.com/user/repo/pull/123";
        let text_content = "●";
        let hyperlinked = format!(
            "{}{}{}",
            osc8::Hyperlink::new(url),
            text_content,
            osc8::Hyperlink::END
        );

        let styled_str = StyledString::raw(&hyperlinked);
        assert_eq!(
            styled_str.width(),
            1,
            "Hyperlinked '●' should have width 1, not {}",
            styled_str.width()
        );
    }

    #[test]
    fn test_width_strips_sgr_codes() {
        // Text with SGR color codes should have visual width of just the text
        use anstyle::{AnsiColor, Color, Style};
        let green = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
        let colored = format!("{}●{}", green.render(), green.render_reset());

        let styled_str = StyledString::raw(colored);
        assert_eq!(
            styled_str.width(),
            1,
            "Colored '●' should have width 1, not {}",
            styled_str.width()
        );
    }

    #[test]
    fn test_width_with_combined_ansi_codes() {
        // Text with both color and hyperlink
        use anstyle::{AnsiColor, Color, Style};
        let url = "https://example.com";
        let yellow = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
        let combined = format!(
            "{}{}● passed{}{}",
            yellow.render(),
            osc8::Hyperlink::new(url),
            osc8::Hyperlink::END,
            yellow.render_reset()
        );

        let styled_str = StyledString::raw(&combined);
        // "● passed" = 1 + 1 (space) + 6 = 8
        assert_eq!(
            styled_str.width(),
            8,
            "Combined styled text should have width 8, not {}",
            styled_str.width()
        );
    }
}
