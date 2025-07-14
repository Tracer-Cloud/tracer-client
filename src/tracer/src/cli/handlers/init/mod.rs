pub mod arguments;
mod handler;
pub(super) mod linux;
pub(super) mod macos;

pub use handler::init;
