//! The schema set, embedded in the binary, and the validation of a document against it.

use std::sync::LazyLock;

use anyhow::{bail, Result};
use jsonschema::{Retrieve, Uri, Validator};
use serde_json::Value;

pub const COMMON: &str = include_str!("../schemas/common/v2/media.schema.json");
pub const FIRMWARE: &str = include_str!("../schemas/firmware/v2/firmware.schema.json");
pub const APP: &str = include_str!("../schemas/app/v2/app.schema.json");

/// Which schema of the set a document follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Firmware,
    App,
}

impl Kind {
    fn text(self) -> &'static str {
        match self {
            Kind::Firmware => FIRMWARE,
            Kind::App => APP,
        }
    }

    /// The word for the kind in a message.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Firmware => "firmware",
            Kind::App => "app",
        }
    }
}

/// The three schemas, parsed once.
static SCHEMAS: LazyLock<Result<Vec<Value>, String>> = LazyLock::new(|| {
    [COMMON, FIRMWARE, APP]
        .iter()
        .map(|t| serde_json::from_str(t).map_err(|e| e.to_string()))
        .collect()
});

/// Serves the schemas of the set by their `$id`, so a `$ref` between two schemas resolves
/// with no file and no network.
struct Embedded;

impl Retrieve for Embedded {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> std::result::Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let wanted = uri.as_str();
        let schemas = SCHEMAS.as_ref().map_err(|e| e.clone())?;
        for v in schemas {
            if v.get("$id").and_then(Value::as_str) == Some(wanted) {
                return Ok(v.clone());
            }
        }
        Err(format!("the schema set has no schema with the id {wanted}").into())
    }
}

fn build_validator(kind: Kind) -> Result<Validator, String> {
    let schema: Value = serde_json::from_str(kind.text())
        .map_err(|e| format!("the {} schema is not JSON: {e}", kind.name()))?;
    jsonschema::options()
        .with_retriever(Embedded)
        .build(&schema)
        .map_err(|e| format!("the {} schema does not compile: {e}", kind.name()))
}

static FIRMWARE_VALIDATOR: LazyLock<Result<Validator, String>> =
    LazyLock::new(|| build_validator(Kind::Firmware));
static APP_VALIDATOR: LazyLock<Result<Validator, String>> =
    LazyLock::new(|| build_validator(Kind::App));

/// The validator of a kind, built once for the process.
fn validator(kind: Kind) -> Result<&'static Validator> {
    let v = match kind {
        Kind::Firmware => &*FIRMWARE_VALIDATOR,
        Kind::App => &*APP_VALIDATOR,
    };
    v.as_ref().map_err(|e| anyhow::anyhow!("{e}"))
}

/// Validates a document. The error lists each violation with its path in the document.
pub fn validate(kind: Kind, doc: &Value, what: &str) -> Result<()> {
    let v = validator(kind)?;
    let errors: Vec<String> = v
        .iter_errors(doc)
        .map(|e| format!("  at {}: {}", e.instance_path(), e))
        .collect();
    if errors.is_empty() {
        return Ok(());
    }
    bail!(
        "{what} does not follow the {} schema:\n{}",
        kind.name(),
        errors.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_follows_the_firmware_schema() {
        let doc: Value = serde_json::from_str(include_str!(
            "../schemas/firmware/v2/examples/autococker-26.29.0.json"
        ))
        .unwrap();
        validate(Kind::Firmware, &doc, "the example").unwrap();
    }

    #[test]
    fn a_back_row_with_a_predicate_is_rejected() {
        let mut doc: Value = serde_json::from_str(include_str!(
            "../schemas/firmware/v2/examples/autococker-26.29.0.json"
        ))
        .unwrap();
        let items = doc["screens"]["root"]["items"].as_array_mut().unwrap();
        items.last_mut().unwrap()["hidden_when"] = Value::from("ramping_off");
        let err = validate(Kind::Firmware, &doc, "the example").unwrap_err();
        assert!(err.to_string().contains("/screens/root/items/"));
    }

    #[test]
    fn a_media_item_of_an_unknown_kind_is_valid() {
        let mut doc: Value = serde_json::from_str(include_str!(
            "../schemas/firmware/v2/examples/autococker-26.29.0.json"
        ))
        .unwrap();
        doc["media"] = serde_json::json!({
            "board": { "kind": "hologram", "src": "pcb:board.holo", "caption": [{"lang": "en", "text": "The board."}] }
        });
        validate(Kind::Firmware, &doc, "the example").unwrap();
        doc["media"]["board"]["kind"] = Value::from("Not A Kind");
        assert!(validate(Kind::Firmware, &doc, "the example").is_err());
    }

    #[test]
    fn only_an_image_has_a_dark_file() {
        let mut doc: Value = serde_json::from_str(include_str!(
            "../schemas/firmware/v2/examples/autococker-26.29.0.json"
        ))
        .unwrap();
        assert_eq!(doc["media"]["buttons"]["kind"], "image");
        assert!(doc["media"]["buttons"]["src_dark"].is_string());
        doc["media"]["buttons"]["kind"] = Value::from("video");
        let err = validate(Kind::Firmware, &doc, "the example").unwrap_err();
        assert!(format!("{err:#}").contains("src_dark"), "{err:#}");
    }
}
