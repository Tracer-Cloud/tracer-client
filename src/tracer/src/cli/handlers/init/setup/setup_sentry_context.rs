use super::super::arguments::FinalizedInitArgs;
use crate::utils::Sentry;
use serde_json::Value;

/// Sets up Sentry context with init arguments and tags
pub fn setup_sentry_context(args: &FinalizedInitArgs) -> anyhow::Result<()> {
    // Layer tags on top of args
    let mut json_args = serde_json::to_value(args)?.as_object().unwrap().clone();
    let tags_json = serde_json::to_value(&args.tags)?
        .as_object()
        .unwrap()
        .clone();
    json_args.extend(tags_json);

    Sentry::add_context("Init Arguments", Value::Object(json_args));
    Sentry::add_tag("user_id", args.tags.user_id.as_ref().unwrap());
    Sentry::add_tag("pipeline_name", &args.pipeline_name);

    Ok(())
}
