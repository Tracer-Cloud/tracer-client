mod binary_replacement;
mod download;
mod process_manager;
mod updater;

pub use updater::update;

#[cfg(test)]
mod tests;
