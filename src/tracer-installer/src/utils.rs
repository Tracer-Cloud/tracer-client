use crate::types::TracerVersion;
use colored::Colorize;

pub enum TagColor {
    Green,
    Red,
    Blue,
    Cyan,
}

pub fn print_message(tag: &str, message: &str, color: TagColor) {
    let tag = format!("[{tag}]");
    let tag = match color {
        TagColor::Green => tag.green(),
        TagColor::Red => tag.red(),
        TagColor::Blue => tag.blue(),
        TagColor::Cyan => tag.cyan(),
    }
    .bold();
    const PADDING: usize = 9;
    let padded = format!("{tag:>width$}", width = PADDING);
    println!("{padded} {message}");
}
pub fn print_status(tag: &str, label: &str, reason: &str, color: TagColor) {
    const PADDING: usize = 30;

    let label = if !reason.is_empty() {
        format!("{}:", label)
    } else {
        label.to_string()
    };
    let padded = format!("{label:<width$}", width = PADDING);
    print_message(tag, format!("{padded}{reason}").as_str(), color);
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
