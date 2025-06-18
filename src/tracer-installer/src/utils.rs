use colored::Colorize;
use console::Emoji;

use crate::types::TracerVersion;

pub enum StepStatus<'a> {
    Success(&'a str),
    Warning(&'a str),
    Error(&'a str),
    Custom(Emoji<'a, 'a>, &'a str), // pass Emoji struct + message
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

        StepStatus::Custom(emoji, reason) => {
            println!("{emoji} {padded}{reason}");
        }
    }
}

pub fn print_summary(label: &str, status: StepStatus) {
    const PASS: Emoji<'_, '_> = Emoji("✅", "[OK]");
    const WARNING: Emoji<'_, '_> = Emoji("⚠️", "[WARN]");
    const FAIL: Emoji<'_, '_> = Emoji("❌", "[X]");

    const PADDING: usize = 40;

    let padded = format!("{label:<width$}", width = PADDING);

    println!(); // spacer before
    match status {
        StepStatus::Success(_) => println!("{PASS} {padded}"),
        StepStatus::Warning(_) => println!("{WARNING} {padded}"),
        StepStatus::Error(_) => println!("{FAIL} {padded}"),
        StepStatus::Custom(emoji, _) => {
            println!("{emoji} {padded}");
        }
    }
    println!(); // spacer after
}

pub fn _print_honey_badger_banner_v2(version: &TracerVersion) {
    println!("                    ___,,___");
    println!("               _,-='=- =-  -`\"--.__,,.._");
    println!("            ,-;// /  - -       -   -= - \"=.");
    println!("          ,'///    -     -   -   =  - ==-=\\`.");
    println!("         |/// /  =    `. - =   == - =.=_,,._ `=/|");
    println!("        ///    -   -    \\  - - = ,ndDMHHMM/\\b  \\\\");
    println!("      ,' - / /        / /\\ =  - /MM(,,._`YQMML  `|");
    println!("     <_,=^Kkm / / / / ///H|wnWWdMKKK#\"\"-;. `\"0\\  |");
    println!("            `\"\"QkmmmmmnWMMM\\\"\"WHMKKMM\\   `--. \\> \\");
    println!("     hjm          `\"\"'  `->>>    ``WHMb,.    `-_<@)");
    println!("                                    `\"QMM`.");
    println!("                                       `>>>");
    println!("{} ", "Tracer Installer".yellow().bold());
    println!(
        "{} {}",
        "Tracer version:".bold(),
        version.to_string().cyan().bold()
    );
}

pub fn print_honey_badger_banner(version: &TracerVersion) {
    println!(" ");
    println!("⠀⠀⠀⠀⠀⠀⠀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│ ");
    println!(
        "⠀⢷⣦⣦⣄⣄⣔⣿⣿⣆⣄⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│ {}",
        "Tracer.bio CLI Installer".bold()
    );
    println!("⠀⠀⠻⣿⣿⣿⣿⣿⣿⣿⣿⠛⣿⣷⣦⡄⡀⠀⠀⠀⠀⠀⠀⠀⠀│ ");
    println!("⠀⠀⠀⠈⠻⣻⣿⣿⣿⣿⣿⣷⣷⣿⣿⣿⣷⣧⡄⡀⠀⠀⠀⠀⠀│ ");
    println!(
        "⠀⠀⠀⠀⠀⠀⠘⠉⠃⠑⠁⠃⠋⠋⠛⠟⢿⢿⣿⣷⣦⡀⠀⠀⠀│ Tracer version: {}",
        version.to_string().blue().bold()
    );
    println!("⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠑⠙⠻⠿⣧⠄⠀│ ");
    println!("⠀          ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠀⠀│ ");
    println!(" ");
}
