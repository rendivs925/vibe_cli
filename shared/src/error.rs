use std::fmt;

#[derive(Debug, Clone)]
pub struct AppError {
    pub message: String,
}

impl AppError {
    pub fn new(message: String) -> Self {
        Self { message }
    }

    pub fn domain(message: String) -> Self {
        Self::new(format!("Domain error: {}", message))
    }

    pub fn storage(message: String) -> Self {
        Self::new(format!("Storage error: {}", message))
    }

    pub fn network(message: String) -> Self {
        Self::new(format!("Network error: {}", message))
    }

    pub fn validation(message: String) -> Self {
        Self::new(format!("Validation error: {}", message))
    }

    pub fn serialization(message: String) -> Self {
        Self::new(format!("Serialization error: {}", message))
    }

    pub fn safety(message: String) -> Self {
        Self::new(format!("Safety error: {}", message))
    }

    pub fn not_found(message: String) -> Self {
        Self::new(format!("Not found: {}", message))
    }

    pub fn ai(message: String) -> Self {
        Self::new(format!("AI error: {}", message))
    }

    pub fn io(message: String) -> Self {
        Self::new(format!("IO error: {}", message))
    }

    pub fn config(message: String) -> Self {
        Self::new(format!("Configuration error: {}", message))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::new(err.to_string())
    }
}
