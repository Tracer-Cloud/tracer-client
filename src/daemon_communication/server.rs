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

use crate::daemon_communication::structs::RunData;
use crate::tracer_client::Message;
use crate::{
    config_manager::{Config, ConfigManager},
    daemon_communication::structs::{InfoResponse, InnerInfoResponse},
    events::{recorder::EventType, send_alert_event, send_log_event, send_update_tags_event},
    extracts::process_watcher::ShortLivedProcessLog,
    run,
    tracer_client::TracerClient,
    utils::{debug_log::Logger, upload::upload_from_file_path},
};
use actix_web::web::Data;
use actix_web::{web, App, Error, HttpResponse, HttpServer};
use serde::Deserialize;

type ProcessOutput<'a> =
    Option<Pin<Box<dyn Future<Output = Result<String, anyhow::Error>> + 'a + Send>>>;

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

        let response_inner: Option<InnerInfoResponse> =
            guard.get_run_metadata().map(|out| out.into());

        let preview = guard.process_watcher.preview_targets();
        let preview_len = guard.process_watcher.preview_targets_count();

        let output = InfoResponse::new(preview, preview_len, response_inner);

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

pub fn get_server(
    tracer_client: Arc<Mutex<TracerClient>>,
    cancellation_token: CancellationToken,
    config: Arc<RwLock<Config>>,
    address: &str,
) -> Result<actix_web::dev::Server, anyhow::Error> {
    Ok(HttpServer::new(move || {
        App::new()
            // todo: remove double arc-s
            .app_data(Data::new(tracer_client.clone()))
            .app_data(Data::new(cancellation_token.clone()))
            .app_data(config.clone())
            .service(web::resource("/log").route(web::post().to(log)))
            .service(web::resource("/alert").route(web::post().to(alert)))
            .service(web::resource("/terminate").route(web::post().to(terminate)))
            .service(web::resource("/start").route(web::post().to(start)))
    })
    .bind(address)?
    // to reduce the overhead
    .workers(1)
    .max_connections(5)
    .run())
}

// todo: also move:
// "end" => process_end_run_command(&tracer_client),
// "refresh_config" => process_refresh_config_command(&tracer_client, &config),
// "tag" => process_tag_command(&service_url, &api_key, object),
// "log_short_lived_process" => process_log_short_lived_process_command(&tracer_client, object)
// "info" => process_info_command(&tracer_client, &mut stream),
// "upload" => process_upload_command(&service_url, &api_key, object),

pub async fn terminate(cancellation_token: Data<CancellationToken>) -> Result<HttpResponse, Error> {
    cancellation_token.cancelled().await; // todo: gracefully shutdown
    Ok(HttpResponse::Ok().body("Daemon terminated"))
}

pub async fn log(
    payload: web::Json<Message>,
    tracer_client: Data<Arc<Mutex<TracerClient>>>,
) -> Result<HttpResponse, Error> {
    tracer_client
        .lock()
        .await
        .send_log_event(payload.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body("Log event processed"))
}

pub async fn alert(
    payload: web::Json<Message>,
    tracer_client: Data<Arc<Mutex<TracerClient>>>,
) -> Result<HttpResponse, Error> {
    tracer_client
        .lock()
        .await
        .send_alert_event(payload.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body("Alert event processed"))
}

pub async fn start(tracer_client: Data<Arc<Mutex<TracerClient>>>) -> Result<HttpResponse, Error> {
    {
        let mut _guard = tracer_client.lock().await;

        _guard
            .start_new_run(None)
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;

        let run_data = _guard.get_run_metadata().map(|r| RunData {
            run_name: r.name,
            run_id: r.id,
            pipeline_name: _guard.get_pipeline_name().to_string(),
        });

        Ok(HttpResponse::Ok().json(run_data))
    }
}
