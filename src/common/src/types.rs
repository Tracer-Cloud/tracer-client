use std::sync::Arc;
use tokio::sync::RwLock;

pub type LinesBufferArc = Arc<RwLock<Vec<String>>>;
