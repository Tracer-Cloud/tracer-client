pub mod current_run;
pub mod event;
pub mod extracts;
pub mod pipeline_tags;
pub mod ebpf_trigger;

use std::sync::Arc;
use tokio::sync::RwLock;

pub type LinesBufferArc = Arc<RwLock<Vec<String>>>;
