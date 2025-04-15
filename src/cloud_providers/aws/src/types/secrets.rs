#[derive(Debug, serde::Deserialize)]
pub struct DatabaseAuth {
    pub username: String,
    pub password: String,
}
