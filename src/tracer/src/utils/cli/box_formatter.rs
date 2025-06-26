use colored::Colorize;
use console::Emoji;
use std::fmt::Write;

const STATUS_ACTIVE: Emoji<'_, '_> = Emoji("🟢 ", "🟢 ");
const STATUS_INACTIVE: Emoji<'_, '_> = Emoji("🔴 ", "🔴 ");
const STATUS_WARNING: Emoji<'_, '_> = Emoji("🟡 ", "🟡 ");
const STATUS_INFO: Emoji<'_, '_> = Emoji("ℹ️ ", "ℹ️ ");

pub struct BoxFormatter {
    output: String,
    width: usize,
    macos_terminal: bool,
}

/// Formats a box like interface in the command line.
/// create with `BoxFormatter::new(width)`
impl BoxFormatter {
    pub fn new(width: usize) -> Self {
        // Check if running in Terminal.app
        let macos_terminal = matches!(
            std::env::var("TERM_PROGRAM").as_deref(),
            Ok("Apple_Terminal")
        );
        let width = if macos_terminal { width + 100 } else { width };
        Self {
            output: String::new(),
            width,
            macos_terminal,
        }
    }

    pub fn add_header(&mut self, title: &str) {
        writeln!(
            &mut self.output,
            "\n┌{:─^width$}┐",
            format!(" {} ", title),
            width = self.width - 2
        )
        .unwrap();
    }

    pub fn add_footer(&mut self) {
        writeln!(
            &mut self.output,
            "└{:─^width$}┘",
            "",
            width = self.width - 2
        )
        .unwrap();
    }

    pub fn add_section_header(&mut self, title: &str) {
        writeln!(
            &mut self.output,
            "├{:─^width$}┤",
            format!(" {} ", title),
            width = self.width - 2
        )
        .unwrap();
    }

    pub fn add_field(&mut self, label: &str, value: &str, color: &str) {
        let colored_value = match color {
            "green" => value.green(),
            "yellow" => value.yellow(),
            "cyan" => value.cyan(),
            "magenta" => value.magenta(),
            "blue" => value.blue(),
            "red" => value.red(),
            "bold" => value.bold(),
            "white" => value.white(),
            _ => value.normal(),
        };

        // Calculate available space for value
        let label_width = 20;
        let padding = 4;
        let max_value_width = self.width - label_width - padding;

        let formatted_value = if value.starts_with("http") {
            colored_value.to_string()
        } else if colored_value.len() > max_value_width {
            format!("{}...", &colored_value[..max_value_width - 3])
        } else {
            colored_value.to_string()
        };

        writeln!(
            &mut self.output,
            "│ {:<label_width$} │ {}  ",
            label, formatted_value
        )
        .unwrap();
    }

    pub fn add_status_field(&mut self, label: &str, value: &str, status: &str) {
        let (emoji, color) = match status {
            "active" => (STATUS_ACTIVE, "green"),
            "inactive" => (STATUS_INACTIVE, "red"),
            "warning" => (STATUS_WARNING, "yellow"),
            _ => (STATUS_INFO, "blue"),
        };

        writeln!(
            &mut self.output,
            "│ {:<20} │ {} {}  ",
            label,
            emoji,
            value.color(color)
        )
        .unwrap();
    }

    pub fn add_empty_line(&mut self) {
        writeln!(&mut self.output, "│{:width$}│", "", width = self.width - 2).unwrap();
    }

    pub fn get_output(&self) -> &str {
        &self.output
    }
    pub fn add_hyperlink(&mut self, label: &str, url: &str, display_text: &str) {
        let link = if self.macos_terminal {
            // Terminal.app: show plain blue URL
            format!("🔗 {}", url).blue().to_string()
        } else {
            // Other terminals: clickable hyperlink
            let display_with_indicator = format!("🔗 {}", display_text);
            let hyperlink = format!("\x1B]8;;{}\x07{}\x1B]8;;\x07", url, display_with_indicator);
            hyperlink.blue().to_string()
        };

        writeln!(&mut self.output, "│ {:<20} │ {}  ", label, link).unwrap();
    }
}
