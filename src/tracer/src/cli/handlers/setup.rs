use crate::config::Config;

pub async fn setup(
    api_key: &Option<String>,
    process_polling_interval_ms: &Option<u64>,
    batch_submission_interval_ms: &Option<u64>,
) -> anyhow::Result<()> {
    let mut current_config = Config::default();
    if let Some(api_key) = api_key {
        current_config.api_key.clone_from(api_key);
    }
    if let Some(process_polling_interval_ms) = process_polling_interval_ms {
        current_config.process_polling_interval_ms = *process_polling_interval_ms;
    }
    if let Some(batch_submission_interval_ms) = batch_submission_interval_ms {
        current_config.batch_submission_interval_ms = *batch_submission_interval_ms;
    }

    //ConfigLoader::save_config(&current_config)?;

    Ok(())
}
