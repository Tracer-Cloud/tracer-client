//use colored::Colorize;
use console::Emoji;

pub enum StepStatus<'a> {
    Success(&'a str),
    Warning(&'a str),
    Error(&'a str),
}

pub fn print_step(label: &str, status: StepStatus) {
    const PASS: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
    const WARNING: Emoji<'_, '_> = Emoji("⚠️ ", "[WARN] ");
    const FAIL: Emoji<'_, '_> = Emoji("❌ ", "[X] ");

    const PADDING: usize = 40; // adjust to keep things aligned

    let padded = format!("{label:<width$}", width = PADDING);

    match status {
        StepStatus::Success(reason) => {
            println!("{PASS} {padded}{reason}");
        }
        StepStatus::Warning(reason) => {
            println!("{WARNING} {padded}{reason}");
        }
        StepStatus::Error(reason) => {
            println!("{FAIL} {padded}{reason}");
        }
    }
}
