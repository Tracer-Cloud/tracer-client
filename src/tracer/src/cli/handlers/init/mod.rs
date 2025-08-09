pub mod arguments;
mod handler;
mod spawn;

pub use handler::{init, init_with_default_prompt};
pub use spawn::spawn_child;
#[cfg(not(target_os = "linux"))]
pub use spawn::resolve_exe_path;