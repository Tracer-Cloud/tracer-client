use anyhow::Result;
use chrono::Utc;
use core::panic;
use serde_json::{json, Value};
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::{Mutex, RwLock},
};
use tokio_util::sync::CancellationToken;

use crate::{
    config_manager::{Config, ConfigManager},
    events::{recorder::EventType, send_alert_event, send_log_event, send_update_tags_event},
    extracts::process_watcher::ShortLivedProcessLog,
    tracer_client::TracerClient,
    utils::{debug_log::Logger, upload::upload_from_file_path},
};

type ProcessOutput<'a> =
    Option<Pin<Box<dyn Future<Output = Result<String, anyhow::Error>> + 'a + Send>>>;

pub fn process_log_command<'a>(
    service_url: &'a str,
    api_key: &'a str,
    object: &serde_json::Map<String, serde_json::Value>,
    tracer_client: &'a Arc<Mutex<TracerClient>>,
) -> ProcessOutput<'a> {
    if !object.contains_key("message") {
        return None;
    };
    let message = object.get("message").unwrap().as_str().unwrap().to_string();

    async fn fun<'a>(
        tracer_client: &'a Arc<Mutex<TracerClient>>,
        _service_url: &'a str,
        api_key: &'a str,
        message: String,
    ) -> Result<String, anyhow::Error> {
        let event_recorder = &mut tracer_client.lock().await.logs;

        event_recorder.record_event(
            EventType::RunStatusMessage,
            message.clone(),
            None,
            Some(Utc::now()),
        );

        // TODO: remove
        send_log_event(api_key, message).await
    }

    Some(Box::pin(fun(tracer_client, api_key, service_url, message)))
}

pub fn process_alert_command<'a>(
    service_url: &'a str,
    api_key: &'a str,
    object: &serde_json::Map<String, serde_json::Value>,
    tracer_client: &'a Arc<Mutex<TracerClient>>,
) -> ProcessOutput<'a> {
    if !object.contains_key("message") {
        return None;
    };

    let message = object.get("message").unwrap().as_str().unwrap().to_string();

    async fn fun<'a>(
        tracer_client: &'a Arc<Mutex<TracerClient>>,
        _service_url: &'a str,
        _api_key: &'a str,
        message: String,
    ) -> Result<String, anyhow::Error> {
        let event_recorder = &mut tracer_client.lock().await.logs;

        event_recorder.record_event(EventType::Alert, message.clone(), None, Some(Utc::now()));
        // TODO: remove
        send_alert_event(message).await
    }
    Some(Box::pin(fun(tracer_client, service_url, api_key, message)))
}

pub fn process_start_run_command<'a>(
    tracer_client: &'a Arc<Mutex<TracerClient>>,
    stream: &'a mut UnixStream,
) -> ProcessOutput<'a> {
    async fn fun<'a>(
        tracer_client: &'a Arc<Mutex<TracerClient>>,
        stream: &'a mut UnixStream,
    ) -> Result<String, anyhow::Error> {
        tracer_client.lock().await.start_new_run(None).await?;

        let guard = tracer_client.lock().await;

        let info = guard.get_run_metadata();

        let output = if let Some(info) = info {
            json!({
                "run_name": info.name,
                "run_id": info.id,
                "pipeline_name": guard.get_pipeline_name(),
            })
        } else {
            json!({
                "run_name": "",
                "run_id": "",
                "pipeline_name": "",
            })
        };

        stream
            .write_all(serde_json::to_string(&output)?.as_bytes())
            .await?;

        stream.flush().await?;

        Ok("".to_string())
    }

    Some(Box::pin(fun(tracer_client, stream)))
}

pub fn process_info_command<'a>(
    tracer_client: &'a Arc<Mutex<TracerClient>>,
    stream: &'a mut UnixStream,
) -> ProcessOutput<'a> {
    async fn fun<'a>(
        tracer_client: &'a Arc<Mutex<TracerClient>>,
        stream: &'a mut UnixStream,
    ) -> Result<String, anyhow::Error> {
        let guard = tracer_client.lock().await;

        let out = guard.get_run_metadata();

        let output = if let Some(out) = out {
            json!({
                "run_name": out.name,
                "run_id": out.id,
                "pipeline_name": guard.get_pipeline_name(),
            })
        } else {
            json!({
                "run_name": "",
                "run_id": "",
                "pipeline_name": "",
            })
        };

        stream
            .write_all(serde_json::to_string(&output)?.as_bytes())
            .await?;

        stream.flush().await?;

        Ok("".to_string())
    }

    Some(Box::pin(fun(tracer_client, stream)))
}

// NOTE: outputs data
pub fn process_end_run_command(tracer_client: &Arc<Mutex<TracerClient>>) -> ProcessOutput<'_> {
    Some(Box::pin(async move {
        let mut tracer_client = tracer_client.lock().await;
        tracer_client.stop_run().await?;
        Ok("".to_string())
    }))
}

pub fn process_refresh_config_command<'a>(
    tracer_client: &'a Arc<Mutex<TracerClient>>,
    config: &'a Arc<RwLock<Config>>,
) -> ProcessOutput<'a> {
    let config_file = ConfigManager::load_config();

    async fn fun<'a>(
        tracer_client: &'a Arc<Mutex<TracerClient>>,
        config: &'a Arc<RwLock<Config>>,
        config_file: crate::config_manager::Config,
    ) -> Result<String, anyhow::Error> {
        tracer_client.lock().await.reload_config_file(&config_file);
        config.write().await.clone_from(&config_file);
        Ok("".to_string())
    }

    Some(Box::pin(fun(tracer_client, config, config_file)))
}

// TODO: should this be an event ?
pub fn process_tag_command<'a>(
    service_url: &'a str,
    api_key: &'a str,
    object: &serde_json::Map<String, serde_json::Value>,
) -> ProcessOutput<'a> {
    if !object.contains_key("tags") {
        return None;
    };

    let tags_json = object.get("tags").unwrap().as_array().unwrap();

    let tags: Vec<String> = tags_json
        .iter()
        .map(|tag| tag.as_str().unwrap().to_string())
        .collect();

    Some(Box::pin(send_update_tags_event(service_url, api_key, tags)))
}

pub fn process_log_short_lived_process_command<'a>(
    tracer_client: &'a Arc<Mutex<TracerClient>>,
    object: &serde_json::Map<String, serde_json::Value>,
) -> ProcessOutput<'a> {
    if !object.contains_key("log") {
        return None;
    };

    let log: ShortLivedProcessLog =
        serde_json::from_value(object.get("log").unwrap().clone()).unwrap();

    Some(Box::pin(async move {
        let mut tracer_client = tracer_client.lock().await;
        tracer_client.fill_logs_with_short_lived_process(log)?;
        Ok("".to_string())
    }))
}

pub fn process_upload_command<'a>(
    service_url: &'a str,
    api_key: &'a str,
    object: &'a serde_json::Map<String, serde_json::Value>,
) -> ProcessOutput<'a> {
    if !object.contains_key("file_path") {
        return None;
    };

    Some(Box::pin(async move {
        let logger = Logger::new();

        logger.log("server.rs//process_upload_command", None).await;

        upload_from_file_path(
            service_url,
            api_key,
            object.get("file_path").unwrap().as_str().unwrap(),
            None,
        )
        .await?;

        logger.log("process_upload_command completed", None).await;
        Ok("Upload command processed".to_string())
    }))
}

pub async fn run_server(
    tracer_client: Arc<Mutex<TracerClient>>,
    socket_path: &str,
    cancellation_token: CancellationToken,
    config: Arc<RwLock<Config>>,
) -> Result<(), anyhow::Error> {
    if std::fs::metadata(socket_path).is_ok() {
        std::fs::remove_file(socket_path)
            .unwrap_or_else(|_| panic!("Failed to remove existing socket file"));
    }
    let listener = UnixListener::bind(socket_path).expect("Failed to bind to unix socket");
    loop {
        let (mut stream, _) = listener.accept().await.unwrap();

        let mut message = String::new();

        let logger = Logger::new();

        let result = stream.read_to_string(&mut message).await;

        if result.is_err() {
            eprintln!("Error reading from socket: {}", result.err().unwrap());
            continue;
        }

        let json_parse_result = serde_json::from_str(&message);

        if json_parse_result.is_err() {
            eprintln!("Error parsing JSON: {}", json_parse_result.err().unwrap());
            continue;
        }

        let parsed: Value = json_parse_result.unwrap();

        if !parsed.is_object() {
            eprintln!("Invalid JSON received: {}", message);
            continue;
        }

        let object = parsed.as_object().unwrap();

        if !object.contains_key("command") {
            eprintln!("Invalid JSON, no command field, received: {}", message);
            continue;
        }

        let command = object.get("command").unwrap().as_str().unwrap();

        let (service_url, api_key) = {
            let tracer_client = tracer_client.lock().await;
            let service_url = tracer_client.get_service_url().to_owned();
            let api_key = tracer_client.get_api_key().to_owned();
            (service_url, api_key)
        };

        logger
            .log(&format!("Received command: {}, {}", command, message), None)
            .await;

        let result = match command {
            "terminate" => {
                cancellation_token.cancel();
                return Ok(());
            }
            "log" => process_log_command(&service_url, &api_key, object, &tracer_client),
            "alert" => process_alert_command(&service_url, &api_key, object, &tracer_client),
            "start" => process_start_run_command(&tracer_client, &mut stream),
            "end" => process_end_run_command(&tracer_client),
            "refresh_config" => process_refresh_config_command(&tracer_client, &config),
            "tag" => process_tag_command(&service_url, &api_key, object),
            "log_short_lived_process" => {
                process_log_short_lived_process_command(&tracer_client, object)
            }
            "info" => process_info_command(&tracer_client, &mut stream),
            "upload" => process_upload_command(&service_url, &api_key, object),
            _ => {
                eprintln!("Invalid command: {}", command);
                None
            }
        };

        if let Some(future) = result {
            future.await?;
        }
    }
}
