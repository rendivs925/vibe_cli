use bincode::{deserialize, serialize};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use shared::types::Result;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::cache::types::COMPRESSION_THRESHOLD_BYTES;

pub struct Storage {
    cache_dir: PathBuf,
}

impl Storage {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    fn cache_path(&self, name: &str) -> PathBuf {
        let mut path = self.cache_dir.clone();
        path.push(format!("{}.cache", name));
        path
    }

    pub fn load<T>(&self, name: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        let path = self.cache_path(name);
        if !path.exists() {
            return Ok(T::default());
        }

        let data = std::fs::read(&path)?;

        if data.first().map(|&b| b == 0x1f).unwrap_or(false) {
            let decoder = GzDecoder::new(&data[..]);
            let mut decoder = decoder;
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            let cache: T = deserialize(&decompressed).unwrap_or_default();
            Ok(cache)
        } else {
            let cache: T = deserialize(&data).unwrap_or_default();
            Ok(cache)
        }
    }

    pub fn save<T: serde::Serialize>(&self, name: &str, data: &T) -> Result<()> {
        let path = self.cache_path(name);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serialize(data)?;

        if serialized.len() > COMPRESSION_THRESHOLD_BYTES {
            let file = File::create(&path)?;
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder.write_all(&serialized)?;
            encoder.finish()?;
        } else {
            std::fs::write(&path, serialized)?;
        }

        Ok(())
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        let path = self.cache_path(name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
