use console::Emoji;
use dialoguer::theme::ColorfulTheme;
use std::sync::LazyLock;

pub static INTERACTIVE_THEME: LazyLock<ColorfulTheme> = LazyLock::new(|| {
    let arrow = Emoji("👉 ", "> ").to_string();
    ColorfulTheme {
        prompt_prefix: dialoguer::console::Style::new().green().apply_to(arrow),
        prompt_suffix: dialoguer::console::Style::new()
            .dim()
            .apply_to(":".to_string()),
        success_prefix: dialoguer::console::Style::new()
            .green()
            .apply_to("✔".to_string()),
        success_suffix: dialoguer::console::Style::new()
            .dim()
            .apply_to("".to_string()),
        values_style: dialoguer::console::Style::new().yellow(),
        active_item_style: dialoguer::console::Style::new().cyan().bold(),
        ..ColorfulTheme::default()
    }
});
