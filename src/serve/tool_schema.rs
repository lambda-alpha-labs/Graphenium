//! JSON Schema sanitization for strict function-calling APIs (Gemini / Vertex AI).
//!
//! Schemars represents `Option<T>` as nullable unions (`type: ["number", "null"]`
//! or `anyOf`). Gemini rejects parameter schemas where `anyOf`/`oneOf`/`allOf`
//! appear alongside sibling keys such as `description`, `type`, or `format`.
//! Optional MCP parameters should use a plain type and be omitted from `required`.

use rmcp::model::JsonObject;
use serde_json::Value;

const UNION_KEYS: [&str; 3] = ["anyOf", "oneOf", "allOf"];

/// Flatten nullable unions in a tool `inputSchema` so Gemini/Vertex can accept it.
pub fn sanitize_tool_input_schema(schema: &mut JsonObject) {
    if let Some(Value::Object(properties)) = schema.get_mut("properties") {
        for property in properties.values_mut() {
            sanitize_property_schema(property);
        }
    }
}

fn sanitize_property_schema(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    for key in UNION_KEYS {
        if obj.contains_key(key) {
            flatten_nullable_union(obj, key);
            break;
        }
    }

    if let Some(Value::Array(types)) = obj.get("type").cloned() {
        let non_null: Vec<Value> = types.into_iter().filter(|t| !is_null_type(t)).collect();
        match non_null.len() {
            0 => {
                obj.remove("type");
            }
            1 => {
                obj.insert("type".to_string(), non_null[0].clone());
            }
            _ => {
                obj.insert("type".to_string(), Value::Array(non_null));
            }
        }
    }
}

fn flatten_nullable_union(obj: &mut serde_json::Map<String, Value>, union_key: &str) {
    let Some(Value::Array(branches)) = obj.remove(union_key) else {
        return;
    };

    let parent_description = obj.remove("description");
    let parent_format = obj.remove("format");
    let parent_default = obj.remove("default");
    let parent_title = obj.remove("title");
    obj.remove("type");

    let non_null: Vec<Value> = branches
        .into_iter()
        .filter(|branch| !is_null_schema(branch))
        .collect();

    if non_null.len() == 1 {
        if let Value::Object(mut branch) = non_null.into_iter().next().unwrap() {
            if let Some(desc) = parent_description {
                branch.entry("description".to_string()).or_insert(desc);
            }
            if let Some(fmt) = parent_format {
                branch.entry("format".to_string()).or_insert(fmt);
            }
            if let Some(def) = parent_default {
                branch.entry("default".to_string()).or_insert(def);
            }
            if let Some(title) = parent_title {
                branch.entry("title".to_string()).or_insert(title);
            }
            *obj = branch;
        }
        return;
    }

    obj.clear();
    obj.insert(union_key.to_string(), Value::Array(non_null));
    if let Some(desc) = parent_description {
        if let Some(Value::Object(branch)) = obj
            .get_mut(union_key)
            .and_then(|v| v.as_array_mut())
            .and_then(|arr| arr.first_mut())
        {
            branch.entry("description".to_string()).or_insert(desc);
        }
    }
}

fn is_null_schema(value: &Value) -> bool {
    match value {
        Value::Object(obj) => obj.get("type").is_some_and(|ty| is_null_type(ty)),
        _ => false,
    }
}

fn is_null_type(value: &Value) -> bool {
    match value {
        Value::String(s) => s == "null",
        Value::Array(items) => items.iter().all(is_null_type),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flattens_nullable_number_type_array() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "weight": {
                    "description": "Optional traversal weight override",
                    "format": "double",
                    "type": ["number", "null"]
                }
            },
            "required": ["source"]
        })
        .as_object()
        .unwrap()
        .clone();

        sanitize_tool_input_schema(&mut schema);

        let weight = &schema["properties"]["weight"];
        assert_eq!(weight["type"], "number");
        assert_eq!(weight["format"], "double");
        assert_eq!(weight["description"], "Optional traversal weight override");
        assert!(weight.get("anyOf").is_none());
    }

    #[test]
    fn flattens_nullable_string_type_array() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "source_location": {
                    "description": "Optional source location hint",
                    "type": ["string", "null"]
                }
            },
            "required": ["source"]
        })
        .as_object()
        .unwrap()
        .clone();

        sanitize_tool_input_schema(&mut schema);

        let loc = &schema["properties"]["source_location"];
        assert_eq!(loc["type"], "string");
        assert_eq!(loc["description"], "Optional source location hint");
    }

    #[test]
    fn flattens_any_of_nullable_union() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "weight": {
                    "description": "Edge weight",
                    "anyOf": [
                        { "type": "number", "format": "double" },
                        { "type": "null" }
                    ]
                }
            }
        })
        .as_object()
        .unwrap()
        .clone();

        sanitize_tool_input_schema(&mut schema);

        let weight = &schema["properties"]["weight"];
        assert_eq!(weight["type"], "number");
        assert_eq!(weight["format"], "double");
        assert_eq!(weight["description"], "Edge weight");
        assert!(weight.get("anyOf").is_none());
    }

    #[test]
    fn add_edge_schema_has_no_nullable_unions_after_sanitize() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "confidence": { "type": "string" },
                "relation": { "type": "string" },
                "source": { "type": "string" },
                "source_file": { "type": "string" },
                "source_location": {
                    "description": "Optional source location hint",
                    "type": ["string", "null"]
                },
                "target": { "type": "string" },
                "weight": {
                    "description": "Optional traversal weight override",
                    "format": "double",
                    "type": ["number", "null"]
                }
            },
            "required": ["confidence", "relation", "source", "source_file", "target"]
        })
        .as_object()
        .unwrap()
        .clone();

        sanitize_tool_input_schema(&mut schema);

        for (name, prop) in schema["properties"].as_object().unwrap() {
            let obj = prop.as_object().unwrap();
            assert!(
                !obj.contains_key("anyOf"),
                "property {name} still has anyOf"
            );
            if let Some(Value::Array(types)) = obj.get("type") {
                assert!(
                    !types.iter().any(|t| t == "null"),
                    "property {name} still allows null in type array"
                );
            }
        }
    }
}
