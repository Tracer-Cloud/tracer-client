use colored::Colorize;
use console::Emoji;

use crate::types::TracerVersion;

#[derive(Debug, Clone, Copy)]
pub enum PrintEmoji {
    Pass,
    Warning,
    Fail,
    OS,
    Arch,
    Cpu,
    Ram,
    Extract,
    Updated,
    Next,
    Downloading,
}
impl PrintEmoji {
    pub fn to_emoji(self) -> Emoji<'static, 'static> {
        match self {
            PrintEmoji::Pass => Emoji("✅ ", "[OK] "),
            PrintEmoji::Warning => Emoji("⚠️ ", "[WARN] "),
            PrintEmoji::Fail => Emoji("❌ ", "[X] "),
            PrintEmoji::OS => Emoji("🐧 ", "[OS]"),
            PrintEmoji::Arch => Emoji("💻 ", "[ARCH]"),
            PrintEmoji::Cpu => Emoji("⚙️ ", "[CPU]"),
            PrintEmoji::Ram => Emoji("💾 ", "[RAM]"),
            PrintEmoji::Extract => Emoji("📂 ", "[DONE]"),
            PrintEmoji::Updated => Emoji("🔄 ", "[UPDATED]"),
            PrintEmoji::Next => Emoji("🚀 ", "[NEXT]"),
            PrintEmoji::Downloading => Emoji("📥 ", "[DOWNLOADING]"),
        }
    }
}
pub fn print_status(label: &str, reason: &str, emoji: PrintEmoji) {
    const PADDING: usize = 40;

    let label = format!("{}:", label);
    let padded = format!("{label:<width$}", width = PADDING);
    let emoji = emoji.to_emoji();
    println!("{emoji} {padded}{reason}");
}

pub fn print_label(label: &str, emoji: PrintEmoji) {
    print_status(label, "", emoji);
}
pub fn print_summary(label: &str, emoji: PrintEmoji) {
    println!(); // spacer before
    print_status(label, "", emoji);
    println!(); // spacer after
}

pub fn _print_anteater_banner_v2(version: &TracerVersion) {
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

pub fn print_anteater_banner(version: &TracerVersion) {
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

pub fn print_title(title: &str) {
    println!("\n==== {} ====\n", title.bold());
}
