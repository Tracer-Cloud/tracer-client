use tracing::subscriber::set_global_default;
use tracing_log::LogTracer;

use tracing_loki::{BackgroundTask, BackgroundTaskController};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;
use url::Url;

pub fn init_tracing() -> Result<(BackgroundTask, BackgroundTaskController), tracing_loki::Error> {
    let loki_url = std::env::var_os("LOKI_OTEL_URL")
        .map(|val| val.to_str().expect("Cannot covert os str").to_string())
        .unwrap_or("http://127.0.0.1:3100".to_string());

    println!("loki url, {}", loki_url);

    LogTracer::init().expect("failed to set logger");

    let (layer, task_controller, task) = tracing_loki::builder()
        .label("host", "tracer-client-rust")?
        .label("pipeline_name", "tracer-client-rust")?
        .extra_field("run_id", uuid::Uuid::new_v4())?
        .build_controller_url(Url::parse(&loki_url).unwrap())?;

    let filter = EnvFilter::new("info");

    let json_layer = tracing_subscriber::fmt::layer()
        .with_level(true)
        .with_ansi(true)
        //.with_span_events(FmtSpan::CLOSE)
        .with_thread_names(true)
        .json();

    let subscriber = tracing_subscriber::registry::Registry::default()
        .with(filter)
        .with(json_layer);

    set_global_default(subscriber.with(layer)).expect("Failed to set default subscriber");

    tracing::info!(
        task = "tracing_setup",
        result = "success",
        "tracing successfully setup"
    );

    Ok((task, task_controller))
}
