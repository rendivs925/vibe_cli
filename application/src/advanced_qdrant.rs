//! Advanced Qdrant Configuration and Optimization
//!
//! This module provides advanced Qdrant features for production deployment,
//! including quantization, HNSW optimization, and performance tuning.

use shared::types::Result;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct QdrantOptimizationConfig {
    pub quantization: Option<QuantizationConfig>,
    pub hnsw_config: HnswConfig,
    pub optimizers_config: OptimizersConfig,
    pub wal_config: WalConfig,
    pub payload_schema: Option<HashMap<String, PayloadIndexInfo>>,
}

#[derive(Debug, Clone)]
pub struct QuantizationConfig {
    pub quantization_type: QuantizationType,
    pub always_ram: Option<bool>,
}

#[derive(Debug, Clone)]
pub enum QuantizationType {
    Int8,
    Uint8,
    Float16,
    PQ { num_bits: u32, max_indexing_threads: u32 },
}

#[derive(Debug, Clone)]
pub struct HnswConfig {
    pub m: u32,              // Number of edges per node in the index graph
    pub ef_construct: u32,   // Size of the dynamic candidate list during construction
    pub full_scan_threshold: u32, // Minimum number of vectors for full scan
    pub max_indexing_threads: u32, // Maximum threads for indexing
    pub on_disk: Option<bool>,     // Store HNSW index on disk
}

#[derive(Debug, Clone)]
pub struct OptimizersConfig {
    pub deleted_threshold: f64,     // Threshold for optimization triggers
    pub vacuum_min_vector_number: u32, // Minimum vectors before vacuum
    pub default_segment_number: u32,    // Default number of segments
    pub max_segment_size: Option<u32>,  // Maximum segment size
    pub memmap_threshold: Option<u32>,  // Memory map threshold
    pub indexing_threshold: u32,        // Threshold for indexing
    pub flush_interval_sec: u64,        // Flush interval in seconds
    pub max_optimization_threads: Option<u32>, // Max optimization threads
}

#[derive(Debug, Clone)]
pub struct WalConfig {
    pub wal_capacity_mb: u32,      // WAL capacity in MB
    pub wal_segments_ahead: u32,   // WAL segments ahead
}

#[derive(Debug, Clone)]
pub struct PayloadIndexInfo {
    pub data_type: PayloadDataType,
    pub params: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone)]
pub enum PayloadDataType {
    Keyword,
    Integer,
    Float,
    Geo,
    Text,
}

pub struct AdvancedQdrantManager {
    base_url: String,
    client: reqwest::Client,
}

impl AdvancedQdrantManager {
    pub fn new(qdrant_url: &str) -> Self {
        Self {
            base_url: qdrant_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Create an optimized collection with advanced configuration
    pub async fn create_optimized_collection(
        &self,
        collection_name: &str,
        vector_dim: usize,
        config: &QdrantOptimizationConfig,
    ) -> Result<()> {
        println!("🚀 Creating optimized collection '{}' with advanced features", collection_name);

        let mut payload = serde_json::json!({
            "vectors": {
                "size": vector_dim,
                "distance": "Cosine"
            },
            "hnsw_config": {
                "m": config.hnsw_config.m,
                "ef_construct": config.hnsw_config.ef_construct,
                "full_scan_threshold": config.hnsw_config.full_scan_threshold,
                "max_indexing_threads": config.hnsw_config.max_indexing_threads
            },
            "optimizers_config": {
                "deleted_threshold": config.optimizers_config.deleted_threshold,
                "vacuum_min_vector_number": config.optimizers_config.vacuum_min_vector_number,
                "default_segment_number": config.optimizers_config.default_segment_number,
                "indexing_threshold": config.optimizers_config.indexing_threshold,
                "flush_interval_sec": config.optimizers_config.flush_interval_sec
            },
            "wal_config": {
                "wal_capacity_mb": config.wal_config.wal_capacity_mb,
                "wal_segments_ahead": config.wal_config.wal_segments_ahead
            }
        });

        // Add optional HNSW on_disk setting
        if let Some(on_disk) = config.hnsw_config.on_disk {
            payload["hnsw_config"]["on_disk"] = serde_json::json!(on_disk);
        }

        // Add optional optimizer settings
        if let Some(max_segment_size) = config.optimizers_config.max_segment_size {
            payload["optimizers_config"]["max_segment_size"] = serde_json::json!(max_segment_size);
        }
        if let Some(memmap_threshold) = config.optimizers_config.memmap_threshold {
            payload["optimizers_config"]["memmap_threshold"] = serde_json::json!(memmap_threshold);
        }
        if let Some(max_threads) = config.optimizers_config.max_optimization_threads {
            payload["optimizers_config"]["max_optimization_threads"] = serde_json::json!(max_threads);
        }

        // Add quantization if specified
        if let Some(quantization) = &config.quantization {
            let quant_config = match &quantization.quantization_type {
                QuantizationType::Int8 => {
                    serde_json::json!({
                        "int8": {
                            "type": "int8"
                        }
                    })
                }
                QuantizationType::Uint8 => {
                    serde_json::json!({
                        "uint8": {
                            "type": "uint8"
                        }
                    })
                }
                QuantizationType::Float16 => {
                    serde_json::json!({
                        "float16": {
                            "type": "float16"
                        }
                    })
                }
                QuantizationType::PQ { num_bits, max_indexing_threads } => {
                    serde_json::json!({
                        "pq": {
                            "type": "pq",
                            "num_bits": num_bits,
                            "max_indexing_threads": max_indexing_threads
                        }
                    })
                }
            };

            payload["quantization_config"] = quant_config;

            if let Some(always_ram) = quantization.always_ram {
                payload["quantization_config"]["always_ram"] = serde_json::json!(always_ram);
            }
        }

        let url = format!("{}/collections/{}", self.base_url, collection_name);

        let response = self.client.put(&url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to create optimized collection: {}", error_text));
        }

        println!("✅ Created optimized collection with advanced features");
        Ok(())
    }

    /// Create payload indexes for faster filtering
    pub async fn create_payload_indexes(
        &self,
        collection_name: &str,
        indexes: &HashMap<String, PayloadIndexInfo>,
    ) -> Result<()> {
        println!("📇 Creating payload indexes for faster filtering");

        for (field_name, index_info) in indexes {
            let payload = match index_info.data_type {
                PayloadDataType::Keyword => {
                    serde_json::json!({
                        "field_name": field_name,
                        "field_schema": {
                            "type": "keyword"
                        }
                    })
                }
                PayloadDataType::Integer => {
                    serde_json::json!({
                        "field_name": field_name,
                        "field_schema": {
                            "type": "integer"
                        }
                    })
                }
                PayloadDataType::Float => {
                    serde_json::json!({
                        "field_name": field_name,
                        "field_schema": {
                            "type": "float"
                        }
                    })
                }
                PayloadDataType::Geo => {
                    serde_json::json!({
                        "field_name": field_name,
                        "field_schema": {
                            "type": "geo"
                        }
                    })
                }
                PayloadDataType::Text => {
                    serde_json::json!({
                        "field_name": field_name,
                        "field_schema": {
                            "type": "text"
                        }
                    })
                }
            };

            let url = format!("{}/collections/{}/index", self.base_url, collection_name);

            let response = self.client.put(&url)
                .json(&payload)
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                eprintln!("⚠️  Failed to create index for field '{}': {}", field_name, error_text);
            } else {
                println!("✅ Created index for field '{}'", field_name);
            }
        }

        Ok(())
    }

    /// Optimize collection for production use
    pub async fn optimize_for_production(&self, collection_name: &str) -> Result<()> {
        println!("🔧 Optimizing collection '{}' for production", collection_name);

        // Force optimization
        let url = format!("{}/collections/{}/optimize", self.base_url, collection_name);

        let response = self.client.post(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            eprintln!("⚠️  Optimization request failed: {}", error_text);
        } else {
            println!("✅ Optimization triggered for collection");
        }

        Ok(())
    }

    /// Get collection performance metrics
    pub async fn get_performance_metrics(&self, collection_name: &str) -> Result<CollectionMetrics> {
        let url = format!("{}/collections/{}", self.base_url, collection_name);

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        let result = data.get("result").ok_or_else(|| anyhow::anyhow!("No result in response"))?;

        let metrics = CollectionMetrics {
            points_count: result.get("points_count").and_then(|v| v.as_u64()).unwrap_or(0),
            segments_count: result.get("segments_count").and_then(|v| v.as_u64()).unwrap_or(0),
            status: result.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            indexed_vectors_count: result.get("indexed_vectors_count").and_then(|v| v.as_u64()).unwrap_or(0),
        };

        Ok(metrics)
    }

    /// Create production-ready semantic memory collection
    pub async fn create_production_semantic_memory_collection(&self, collection_name: &str) -> Result<()> {
        println!("🏭 Creating production-ready semantic memory collection");

        let config = QdrantOptimizationConfig {
            quantization: Some(QuantizationConfig {
                quantization_type: QuantizationType::PQ {
                    num_bits: 8,
                    max_indexing_threads: 4,
                },
                always_ram: Some(false), // Allow disk-based quantization
            }),
            hnsw_config: HnswConfig {
                m: 32,              // Higher connectivity for better recall
                ef_construct: 200,   // Higher construction quality
                full_scan_threshold: 10000,
                max_indexing_threads: 4,
                on_disk: Some(false), // Keep HNSW in memory for speed
            },
            optimizers_config: OptimizersConfig {
                deleted_threshold: 0.2,
                vacuum_min_vector_number: 1000,
                default_segment_number: 0,
                max_segment_size: Some(50000),
                memmap_threshold: Some(100000),
                indexing_threshold: 50000,
                flush_interval_sec: 30, // More frequent flushing
                max_optimization_threads: Some(4),
            },
            wal_config: WalConfig {
                wal_capacity_mb: 64,  // Larger WAL for better performance
                wal_segments_ahead: 2,
            },
            payload_schema: Some({
                let mut schema = HashMap::new();
                schema.insert("conversation_id".to_string(), PayloadIndexInfo {
                    data_type: PayloadDataType::Keyword,
                    params: None,
                });
                schema.insert("timestamp".to_string(), PayloadIndexInfo {
                    data_type: PayloadDataType::Integer,
                    params: None,
                });
                schema
            }),
        };

        // Create the optimized collection
        self.create_optimized_collection(collection_name, 768, &config).await?;

        // Create payload indexes
        if let Some(schema) = &config.payload_schema {
            self.create_payload_indexes(collection_name, schema).await?;
        }

        // Optimize for production
        self.optimize_for_production(collection_name).await?;

        println!("🎯 Production semantic memory collection ready!");
        Ok(())
    }
}

#[derive(Debug)]
pub struct CollectionMetrics {
    pub points_count: u64,
    pub segments_count: u64,
    pub status: String,
    pub indexed_vectors_count: u64,
}

impl Default for QdrantOptimizationConfig {
    fn default() -> Self {
        Self {
            quantization: None,
            hnsw_config: HnswConfig {
                m: 16,
                ef_construct: 100,
                full_scan_threshold: 10000,
                max_indexing_threads: 0,
                on_disk: None,
            },
            optimizers_config: OptimizersConfig {
                deleted_threshold: 0.2,
                vacuum_min_vector_number: 1000,
                default_segment_number: 0,
                max_segment_size: None,
                memmap_threshold: None,
                indexing_threshold: 10000,
                flush_interval_sec: 5,
                max_optimization_threads: None,
            },
            wal_config: WalConfig {
                wal_capacity_mb: 32,
                wal_segments_ahead: 0,
            },
            payload_schema: None,
        }
    }
}