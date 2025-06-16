#[cfg(test)]
mod tests {
    use crate::config::ConfigLoader;

    #[test]
    fn test_default_config() {
        let config = ConfigLoader::load_default_config().unwrap();
        assert!(!config.targets.is_empty());
    }
}