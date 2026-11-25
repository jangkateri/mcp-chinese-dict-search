//! Tool output schemas derived from the serialized response types.
//!
//! The schema and the payload cannot drift because both come from the same
//! `JsonSchema`/`Serialize` derives. Schemars' output is reduced to the narrow
//! subset the tool layer documents: `type`, `properties`, `required`,
//! `additionalProperties`, `items`, `enum`, `const`, plus `description`,
//! `title`, `default`, and `examples`.

use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::{Map, Value};

const ALLOWED: [&str; 11] = [
    "type",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "enum",
    "const",
    "description",
    "title",
    "default",
    "examples",
];

/// The narrow output schema for one tool response type.
pub fn output_schema<T: JsonSchema>() -> Value {
    let settings = SchemaSettings::draft2020_12().with(|settings| {
        settings.inline_subschemas = true;
        settings.meta_schema = None;
    });
    let schema = settings.into_generator().into_root_schema_for::<T>();
    let value = serde_json::to_value(schema).expect("a generated schema serializes");
    narrow(&value, 0)
}

fn narrow(value: &Value, depth: usize) -> Value {
    assert!(depth < 64, "output schema nests too deeply: {value}");
    let Value::Object(object) = value else {
        return value.clone();
    };
    assert!(
        !object.contains_key("$ref"),
        "output schema keeps a reference: {value}"
    );
    if let Some(any_of) = object.get("anyOf").and_then(Value::as_array) {
        let concrete: Vec<&Value> = any_of.iter().filter(|branch| !is_null(branch)).collect();
        assert_eq!(
            concrete.len(),
            1,
            "unsupported union in output schema: {value}"
        );
        return narrow(concrete[0], depth + 1);
    }

    let mut output = Map::new();
    for (key, item) in object {
        match key.as_str() {
            "properties" => {
                let mut properties = Map::new();
                for (name, property) in item.as_object().expect("properties is an object") {
                    properties.insert(name.clone(), narrow(property, depth + 1));
                }
                output.insert("properties".to_owned(), Value::Object(properties));
            }
            "items" => {
                output.insert("items".to_owned(), narrow(item, depth + 1));
            }
            "type" => {
                output.insert("type".to_owned(), without_null(item));
            }
            _ if ALLOWED.contains(&key.as_str()) => {
                output.insert(key.clone(), item.clone());
            }
            _ => {}
        }
    }
    if output.get("type").and_then(Value::as_str) == Some("object")
        && output.contains_key("properties")
    {
        output.insert("additionalProperties".to_owned(), Value::Bool(false));
    }
    Value::Object(output)
}

fn is_null(branch: &Value) -> bool {
    branch.get("type").and_then(Value::as_str) == Some("null")
}

fn without_null(value: &Value) -> Value {
    let Some(types) = value.as_array() else {
        return value.clone();
    };
    let concrete: Vec<Value> = types
        .iter()
        .filter(|kind| kind.as_str() != Some("null"))
        .cloned()
        .collect();
    assert_eq!(concrete.len(), 1, "unsupported type union: {value}");
    concrete[0].clone()
}
