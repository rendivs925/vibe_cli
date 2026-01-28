use serde::{Deserialize, Serialize};

pub type Result<T> = anyhow::Result<T>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}
