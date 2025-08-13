use colored::Colorize;
use std::fmt::Write;
use termion::terminal_size;

pub struct BoxFormatter {
    output: String,
    width: usize,
}

/// Formats a box like interface in the command line.
/// create with `BoxFormatter::new(width)`
impl BoxFormatter {
    pub fn new(width: usize) -> Self {
        Self {
            output: String::new(),
            width: Self::get_width(width),
        }
    }

    /// Create a new BoxFormatter that automatically uses the terminal width
    pub fn new_auto_width() -> Self {
        let terminal_width = Self::get_terminal_width();
        Self {
            output: String::new(),
            width: terminal_width,
        }
    }

    /// Get the terminal width, with a fallback to a reasonable default
    fn get_terminal_width() -> usize {
        match terminal_size() {
            Ok((width, _)) => {
                let width = width as usize;
                // Ensure we have a minimum width and leave some padding
                if width > 10 {
                    width - 2 // Leave 2 characters padding
                } else {
                    80 // Fallback to 80 characters
                }
            }
            Err(_) => 80, // Fallback to 80 characters if terminal size detection fails
        }
    }

    fn get_width(max_width: usize) -> usize {
        let terminal_width = Self::get_terminal_width();
        if max_width > terminal_width {
            terminal_width
        } else {
            max_width
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
        // Calculate available space for value
        let label_width = 20;
        let padding = 4;
        let max_value_width = self.width - label_width - padding;
        let formatted_value = Self::format_value(value, color, max_value_width);
        writeln!(
            &mut self.output,
            "│ {:<label_width$} │ {}  ",
            label, formatted_value
        )
        .unwrap();
    }

    pub fn add_multiline_field(&mut self, label: &str, value: &[String], color: &str) {
        // Calculate available space for value
        let label_width = 20;
        let padding = 4;
        let max_value_width = self.width - label_width - padding;
        let mut formatted_value = value
            .iter()
            .map(|value| Self::format_value(value, color, max_value_width));
        writeln!(
            &mut self.output,
            "│ {:<label_width$} │ {}  ",
            label,
            formatted_value.next().unwrap()
        )
        .unwrap();
        for value in formatted_value {
            writeln!(&mut self.output, "│ {:<label_width$} │ {}  ", "", value).unwrap();
        }
    }

    fn format_value(value: &str, color: &str, max_value_width: usize) -> String {
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
        if value.starts_with("http") {
            colored_value.to_string()
        } else if colored_value.len() > max_value_width {
            format!("{}...", &colored_value[..max_value_width - 3])
        } else {
            colored_value.to_string()
        }
    }

    pub fn add_status_field(&mut self, label: &str, value: &str, status: &str) {
        let (status_label, color) = match status {
            "active" => ("[ACTIVE]", "green"),
            "inactive" => ("[INACTIVE]", "red"),
            "warning" => ("[WARNING]", "yellow"),
            _ => ("[INFO]", "blue"),
        };

        writeln!(
            &mut self.output,
            "│ {:<20} │ {} {}  ",
            label,
            status_label.color(color),
            value
        )
        .unwrap();
    }

    pub fn add_empty_line(&mut self) {
        writeln!(&mut self.output, "│{:width$}│", "", width = self.width - 2).unwrap();
    }

    pub fn get_output(&self) -> &str {
        &self.output
    }
    pub fn add_hyperlink(&mut self, label: &str, url: &str) {
        let link = url.blue().to_string();
        writeln!(&mut self.output, "│ {:<20} │ {}  ", label, link).unwrap();
    }
}
