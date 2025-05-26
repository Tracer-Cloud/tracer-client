use super::ProcessState;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracer_common::types::ebpf_trigger::{ExitReason, OutOfMemoryTrigger, ProcessEndTrigger};
use tracing::debug;
