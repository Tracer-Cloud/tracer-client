use crate::cli::handlers::init_arguments::FinalizedInitArgs;
use crate::client::exporters::event_forward::EventForward;
use crate::client::exporters::event_writer::LogWriterEnum;
use crate::config::Config;

pub(in crate::daemon) async fn get_db_client(
    init_args: &FinalizedInitArgs,
    config: &Config,
) -> LogWriterEnum {
    // if we pass --is-dev=false, we use the prod endpoint
    // if we don't pass any value, we use the prod endpoint
    // if we pass --is-dev=true, we use the dev endpoint
    // dev endpoint points to clickhouse, prod endpoint points to postgres
    let event_forward_endpoint = if init_args.dev {
        println!(
            "Using dev endpoint: {}",
            &config.event_forward_endpoint_dev.as_ref().unwrap()
        );
        &config.event_forward_endpoint_dev.as_ref().unwrap()
    } else {
        println!(
            "Using prod endpoint: {}",
            &config.event_forward_endpoint_prod.as_ref().unwrap()
        );
        &config.event_forward_endpoint_prod.as_ref().unwrap()
    };

    LogWriterEnum::Forward(EventForward::try_new(event_forward_endpoint).await.unwrap())
}
