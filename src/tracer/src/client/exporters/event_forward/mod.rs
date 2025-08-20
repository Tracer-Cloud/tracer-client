//! Event forwarding module for sending events to remote endpoints
//!
//! This module provides a clean, modular implementation for forwarding events
//! to remote HTTP endpoints with proper error handling, retry logic, and telemetry.
//!
//! # Example
//!
//! ```rust,no_run
//! # use anyhow::Result;
//! # use tracer::client::exporters::event_forward::EventForward;
//! # use tracer::client::exporters::event_writer::EventWriter;
//! #
//! # #[tokio::main]
//! # async fn main() -> Result<()> {
//! let forwarder = EventForward::try_new("https://api.example.com/events").await?;
//! let events = vec![]; // Empty events list for example
//! forwarder.batch_insert_events(events).await?;
//! forwarder.close().await?;
//! # Ok(())
//! # }
//! ```

mod client;
mod error;
mod retry;
mod telemetry;

// Public exports
pub use client::EventForward;
pub use error::{EventForwardError, EventForwardResult};
