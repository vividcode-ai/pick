//! Build script for model registry code generation.
//! Reads models/providers/*.json and generates:
//!   1. models_data.json — serialized model registry (loaded at runtime)
//!   2. models_generated.rs — thin wrapper that deserializes the JSON
//!
//! Using JSON + serde avoids a 15K-line initialization function in debug builds,
//! which would overflow the default 1MB stack.

use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=models/providers/");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let providers_dir = manifest_dir.join("models").join("providers");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let output_rs = out_dir.join("models_generated.rs");
    let output_json = out_dir.join("models_data.json");

    // Collect all JSON files
    let entries = match fs::read_dir(&providers_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("warning: could not read models/providers/: {e}");
            write_empty(&output_rs, &output_json);
            return;
        }
    };

    let mut json_files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .map(|e| e.path())
        .collect();
    json_files.sort();

    if json_files.is_empty() {
        write_empty(&output_rs, &output_json);
        return;
    }

    // Build registry as a JSON-serializable structure matching Model's serde repr
    let mut registry: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

    for path in &json_files {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: could not read {:?}: {e}", path);
                continue;
            }
        };

        let provider_json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("warning: invalid JSON in {:?}: {e}", path);
                continue;
            }
        };

        let provider_id = provider_json["provider_id"].as_str().unwrap_or("unknown");
        let default_api = provider_json["api"].as_str().unwrap_or("");
        let default_base_url = provider_json["base_url"].as_str().unwrap_or("");

        let models = provider_json["models"].as_array().unwrap();

        let mut provider_models: serde_json::Map<String, serde_json::Value> =
            serde_json::Map::new();

        for model_val in models {
            let id = model_val["id"].as_str().unwrap_or("unknown");
            let name = model_val["name"].as_str().unwrap_or(id);
            let api = model_val["api"].as_str().unwrap_or(default_api);
            let base_url = model_val["base_url"].as_str().unwrap_or(default_base_url);
            let reasoning = model_val["reasoning"].as_bool().unwrap_or(false);
            let context_window = model_val["context_window"].as_u64().unwrap_or(4096);
            let max_tokens = model_val["max_tokens"].as_u64().unwrap_or(4096);

            // Capabilities
            let caps: Vec<serde_json::Value> = model_val["input_capabilities"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| serde_json::Value::String(s.to_string()))
                        .collect()
                })
                .unwrap_or_else(|| vec![serde_json::Value::String("text".to_string())]);

            // Cost
            let cost_input = model_val["cost"]["input"].as_f64().unwrap_or(0.0);
            let cost_output = model_val["cost"]["output"].as_f64().unwrap_or(0.0);
            let cost_cache_read = model_val["cost"]["cache_read"].as_f64().unwrap_or(0.0);
            let cost_cache_write = model_val["cost"]["cache_write"].as_f64().unwrap_or(0.0);

            // Compat config
            let compat = if let Some(c) = model_val.get("compat") {
                if c.is_null() {
                    serde_json::Value::Null
                } else {
                    c.clone()
                }
            } else {
                serde_json::Value::Null
            };

            // Thinking level map
            let thinking_level_map = if let Some(tlm) = model_val.get("thinking_level_map") {
                if tlm.is_null() {
                    serde_json::Value::Null
                } else if let Some(obj) = tlm.as_object() {
                    let mut tlm_map = serde_json::Map::new();
                    for (k, v) in obj {
                        if v.is_null() {
                            tlm_map.insert(k.clone(), serde_json::Value::Null);
                        } else {
                            tlm_map.insert(
                                k.clone(),
                                serde_json::Value::String(v.as_str().unwrap_or("").to_string()),
                            );
                        }
                    }
                    serde_json::Value::Object(tlm_map)
                } else {
                    serde_json::Value::Null
                }
            } else {
                serde_json::Value::Null
            };

            // Headers
            let headers = if let Some(h) = model_val.get("headers") {
                if let Some(obj) = h.as_object() {
                    let mut h_map = serde_json::Map::new();
                    for (k, v) in obj {
                        h_map.insert(
                            k.clone(),
                            serde_json::Value::String(v.as_str().unwrap_or("").to_string()),
                        );
                    }
                    serde_json::Value::Object(h_map)
                } else {
                    serde_json::Value::Null
                }
            } else {
                serde_json::Value::Null
            };

            let model_entry = serde_json::json!({
                "id": id,
                "name": name,
                "api": api,
                "provider": provider_id,
                "base_url": base_url,
                "reasoning": reasoning,
                "thinking_level_map": thinking_level_map,
                "compat": compat,
                "input_capabilities": caps,
                "cost": {
                    "input": cost_input,
                    "output": cost_output,
                    "cache_read": cost_cache_read,
                    "cache_write": cost_cache_write
                },
                "context_window": context_window,
                "max_tokens": max_tokens,
                "headers": headers,
            });

            provider_models.insert(id.to_string(), model_entry);
        }

        registry.insert(
            provider_id.to_string(),
            serde_json::Value::Object(provider_models),
        );
    }

    // Write JSON data file
    let json_data =
        serde_json::to_string(&serde_json::Value::Object(registry)).unwrap_or_else(|e| {
            panic!("failed to serialize model registry: {e}");
        });
    fs::write(&output_json, &json_data).unwrap_or_else(|e| {
        panic!("failed to write {}: {e}", output_json.display());
    });

    // Write thin Rust file that deserializes at runtime.
    // Note: no `use` imports — this is `include!()`-ed into models.rs which already imports them.
    let rs_content = r#"// This file is auto-generated by build.rs. Do not edit.

/// Global model registry: provider_name -> (model_id -> Model)
pub static MODEL_REGISTRY: LazyLock<HashMap<String, HashMap<String, Model>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("models_data.json"))
        .expect("Failed to parse model registry data (corrupt build output)")
});
"#;
    fs::write(&output_rs, rs_content).unwrap_or_else(|e| {
        panic!("failed to write {}: {e}", output_rs.display());
    });

    let total = json_files
        .iter()
        .filter_map(|p| {
            let content = fs::read_to_string(p).ok()?;
            let val: serde_json::Value = serde_json::from_str(&content).ok()?;
            val["models"].as_array().map(|a| a.len())
        })
        .sum::<usize>();

    eprintln!(
        "Generated model registry ({} providers, {} models)",
        json_files.len(),
        total
    );
}

fn write_empty(rs_path: &PathBuf, json_path: &PathBuf) {
    let rs_content = r#"// This file is auto-generated by build.rs. Do not edit.

/// Global model registry (empty — no model data found)
pub static MODEL_REGISTRY: LazyLock<HashMap<String, HashMap<String, Model>>> = LazyLock::new(|| {
    HashMap::new()
});
"#;
    fs::write(rs_path, rs_content).unwrap_or_else(|e| {
        panic!("failed to write {}: {e}", rs_path.display());
    });
    let _ = fs::write(json_path, "{}");
}
