//! A collection of macros for printing messages to the console with different styles.
#[macro_export]
macro_rules! success_message {
    ($($arg:tt)*) => {
        println!("{} {}", "[SUCCESS]".green().bold(), format!($($arg)*));
    };
}

#[macro_export]
macro_rules! error_message {
    ($($arg:tt)*) => {
        eprintln!("{} {}", "  [ERROR]".red().bold(), format!($($arg)*));
    };
}

#[macro_export]
macro_rules! warning_message {
    ($($arg:tt)*) => {
        println!("{} {}", "[WARNING]".yellow().bold(), format!($($arg)*));
    };
}

#[macro_export]
macro_rules! info_message {
    ($($arg:tt)*) => {
        println!("{} {}", "   [INFO]".cyan().bold(), format!($($arg)*));
    };
}
