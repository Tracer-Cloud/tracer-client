#[derive(Clone, Debug)]
pub(crate) enum TracerVersion {
    Development,
    Production,
    Feature(String),
}
