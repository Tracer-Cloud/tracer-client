pub mod current_run;
pub mod event;
pub mod extracts;
pub mod pipeline_tags;

use std::sync::Arc;
use tokio::sync::RwLock;

pub type LinesBufferArc = Arc<RwLock<Vec<String>>>;
