pub mod presigned_url_put;
pub mod upload_to_signed_url;

use crate::debug_log::Logger;
use crate::http_client::upload::upload_to_signed_url::upload_file_to_signed_url_s3;
use anyhow::{Context, Result};
use presigned_url_put::request_presigned_url;
use std::fs;
use std::path::Path;

pub async fn upload_from_file_path(
    service_url: &str,
    api_key: &str,
    file_path: &str,
    custom_file_name: Option<&str>,
) -> Result<()> {
    // todo: this should be split into getting the files + uploading them to tracer. Uploading should
    // be done in `TracerClient`

    const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024; // 5MB in bytes

    let logger = Logger::new();

    // Step #1: Check if the file exists
    let path = Path::new(file_path);
    if !path.exists() {
        logger
            .log(&format!("The file '{}' does not exist.", file_path), None)
            .await;
        return Err(anyhow::anyhow!("The file '{}' does not exist.", file_path));
    }

    logger
        .log(&format!("The file '{}' exists.", file_path), None)
        .await;

    // Step #2: Extract the file name
    let file_name = if let Some(file_name) = custom_file_name {
        file_name
    } else {
        path.file_name()
            .context("Failed to extract file name")?
            .to_str()
            .context("File name is not valid UTF-8")?
    };

    logger
        .log(&format!("Uploading file '{}'", file_name), None)
        .await;

    // Step #3: Check if the file is under 5MB
    let metadata = fs::metadata(file_path)?;
    let file_size = metadata.len();
    if file_size > MAX_FILE_SIZE {
        println!(
            "Warning: File size ({} bytes) exceeds 5MB limit.",
            file_size
        );
        return Err(anyhow::anyhow!("File size exceeds 5MB limit"));
    }

    logger
        .log(&format!("File size: {} bytes", file_size), None)
        .await;

    // Step #4: Request the upload URL
    let signed_url = request_presigned_url(service_url, api_key, file_name).await?;

    logger
        .log(&format!("Presigned URL: {}", signed_url), None)
        .await;

    // Step #5: Upload the file
    upload_file_to_signed_url_s3(&signed_url, file_path).await?;

    logger.log("File uploaded successfully", None).await;

    // Log success
    println!("File '{}' has been uploaded successfully.", file_name);

    Ok(())
}
