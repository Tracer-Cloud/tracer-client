use std::env;
use crate::common::target_process::target_matching::TargetMatch;
use crate::config::ConfigLoader;

pub fn setup_aliases() -> anyhow::Result<()> {
    let config = ConfigLoader::load_default_config()?;
    crate::config::bashrc_intercept::rewrite_interceptor_bashrc_file(
        env::current_exe()?,
        config
            .targets
            .iter()
            .filter(|target| {
                matches!(
                        &target.match_type,
                        TargetMatch::ShortLivedProcessExecutable(_)
                    )
            })
            .collect(),
    )?;

    crate::config::bashrc_intercept::modify_bashrc_file(".bashrc")?;

    println!("Command interceptors setup successfully.");
    Ok(())
}