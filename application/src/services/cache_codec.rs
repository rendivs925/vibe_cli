use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::de::DeserializeOwned;
use serde::Serialize;
use shared::error::AppError;

pub fn encode_cache<T: Serialize>(value: &T) -> Result<String, AppError> {
    let bytes =
        bincode::serialize(value).map_err(|e| AppError::serialization(e.to_string()))?;
    Ok(STANDARD.encode(bytes))
}

pub fn decode_cache<T: DeserializeOwned>(data: &str) -> Result<T, AppError> {
    let bytes = STANDARD
        .decode(data)
        .map_err(|e| AppError::serialization(e.to_string()))?;
    bincode::deserialize(&bytes).map_err(|e| AppError::serialization(e.to_string()))
}
