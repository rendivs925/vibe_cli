// Output parser for structured command output

use crate::domain_config::types::*;
use std::collections::HashMap;

/// Parser for command output based on output schema
#[derive(Debug, Clone)]
pub struct OutputParser;

impl OutputParser {
    /// Parse command output into structured data
    pub fn parse(
        &self,
        output: &str,
        schema: &OutputSchema,
    ) -> Vec<HashMap<String, serde_json::Value>> {
        if output.trim().is_empty() {
            return Vec::new();
        }

        // Handle JSON format
        if schema.format.as_ref().map(|s| s == "json").unwrap_or(false) {
            return self.parse_json(output);
        }

        // Handle delimited format (CSV, TSV, space-delimited)
        if let Some(delimiter) = &schema.delimiter {
            return self.parse_delimited(output, delimiter, schema);
        }

        // Default: try to parse as key=value pairs
        self.parse_key_value(output, schema)
    }

    /// Parse JSON output
    fn parse_json(&self, output: &str) -> Vec<HashMap<String, serde_json::Value>> {
        // Try to parse as array of objects
        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(output) {
            return arr
                .into_iter()
                .map(|v| match v {
                    serde_json::Value::Object(map) => {
                        map.into_iter().map(|(k, v)| (k, v)).collect()
                    }
                    _ => HashMap::new(),
                })
                .collect();
        }

        // Try to parse as single object
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(output) {
            if let serde_json::Value::Object(map) = obj {
                return vec![map.into_iter().map(|(k, v)| (k, v)).collect()];
            }
        }

        Vec::new()
    }

    /// Parse delimited output (CSV, TSV, etc.)
    fn parse_delimited(
        &self,
        output: &str,
        delimiter: &str,
        schema: &OutputSchema,
    ) -> Vec<HashMap<String, serde_json::Value>> {
        let lines: Vec<&str> = output.lines().collect();
        let mut results = Vec::new();

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = if delimiter == " " {
                // For space-delimited, handle multiple spaces
                line.split_whitespace().collect()
            } else {
                line.split(delimiter).collect()
            };

            if parts.is_empty() {
                continue;
            }

            let mut record = HashMap::new();

            // Map columns to properties
            if let Some(item) = &schema.items {
                for (prop_name, prop) in &item.properties {
                    if let Some(column) = prop.column {
                        if column < parts.len() {
                            let value_str = parts[column].trim();
                            let value = self.parse_value(value_str, &prop.type_);
                            record.insert(prop_name.clone(), value);
                        }
                    }
                }
            }

            if !record.is_empty() {
                results.push(record);
            }
        }

        results
    }

    /// Parse key=value pairs
    fn parse_key_value(
        &self,
        output: &str,
        _schema: &OutputSchema,
    ) -> Vec<HashMap<String, serde_json::Value>> {
        let mut records = Vec::new();
        let mut current = HashMap::new();

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                if !current.is_empty() {
                    records.push(current.clone());
                    current.clear();
                }
                continue;
            }

            // Try to parse "key: value" or "key=value" format
            if let Some(colon_pos) = line.find(": ") {
                let key = &line[..colon_pos];
                let value = &line[colon_pos + 2..];
                current.insert(key.to_string(), self.parse_value(value, "string"));
            } else if let Some(eq_pos) = line.find('=') {
                let key = &line[..eq_pos];
                let value = &line[eq_pos + 1..];
                current.insert(key.to_string(), self.parse_value(value, "string"));
            }
        }

        if !current.is_empty() {
            records.push(current);
        }

        records
    }

    /// Parse a string value into the target type
    fn parse_value(&self, value_str: &str, target_type: &str) -> serde_json::Value {
        match target_type {
            "integer" => {
                if let Ok(n) = value_str.parse::<i64>() {
                    serde_json::Value::Number(n.into())
                } else if let Ok(n) = value_str.parse::<f64>() {
                    if let Some(num) = serde_json::Number::from_f64(n) {
                        serde_json::Value::Number(num)
                    } else {
                        serde_json::Value::String(value_str.to_string())
                    }
                } else {
                    serde_json::Value::String(value_str.to_string())
                }
            }
            "number" | "float" => {
                if let Ok(n) = value_str.parse::<f64>() {
                    if let Some(num) = serde_json::Number::from_f64(n) {
                        serde_json::Value::Number(num)
                    } else {
                        serde_json::Value::String(value_str.to_string())
                    }
                } else {
                    serde_json::Value::String(value_str.to_string())
                }
            }
            "boolean" => {
                if value_str.to_lowercase() == "true" || value_str == "1" {
                    serde_json::Value::Bool(true)
                } else if value_str.to_lowercase() == "false" || value_str == "0" {
                    serde_json::Value::Bool(false)
                } else {
                    serde_json::Value::String(value_str.to_string())
                }
            }
            _ => serde_json::Value::String(value_str.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_delimited() {
        let parser = OutputParser;

        let schema = OutputSchema {
            type_: "array".to_string(),
            items: Some(OutputItem {
                type_: "object".to_string(),
                properties: vec![
                    (
                        "pid".to_string(),
                        OutputProperty {
                            type_: "integer".to_string(),
                            column: Some(0),
                            key: None,
                        },
                    ),
                    (
                        "cmdline".to_string(),
                        OutputProperty {
                            type_: "string".to_string(),
                            column: Some(1),
                            key: None,
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            }),
            properties: HashMap::new(),
            format: None,
            delimiter: Some(" ".to_string()),
        };

        let output = "1234 nginx: master\n5678 python app.py";
        let results = parser.parse(output, &schema);

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].get("pid").unwrap(),
            &serde_json::Value::Number(1234.into())
        );
        assert_eq!(
            results[0].get("cmdline").unwrap(),
            &serde_json::Value::String("nginx:".to_string())
        );
    }

    #[test]
    fn test_parse_json() {
        let parser = OutputParser;

        let schema = OutputSchema {
            type_: "array".to_string(),
            items: None,
            properties: HashMap::new(),
            format: Some("json".to_string()),
            delimiter: None,
        };

        let output = r#"[{"pid": 1234, "cmdline": "nginx"}, {"pid": 5678, "cmdline": "python"}]"#;
        let results = parser.parse(output, &schema);

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].get("pid").unwrap(),
            &serde_json::Value::Number(1234.into())
        );
    }

    #[test]
    fn test_parse_key_value() {
        let parser = OutputParser;

        let schema = OutputSchema {
            type_: "array".to_string(),
            items: None,
            properties: HashMap::new(),
            format: None,
            delimiter: None,
        };

        let output = "Name: nginx\nStatus: running\n\nName: python\nStatus: running";
        let results = parser.parse(output, &schema);

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].get("Name").unwrap(),
            &serde_json::Value::String("nginx".to_string())
        );
        assert_eq!(
            results[0].get("Status").unwrap(),
            &serde_json::Value::String("running".to_string())
        );
    }
}
