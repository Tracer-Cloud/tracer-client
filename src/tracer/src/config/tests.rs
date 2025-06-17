#[cfg(test)]
mod tests {
    use crate::config::Config;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.targets.is_empty());
    }
}
