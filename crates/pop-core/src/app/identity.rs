use super::{AppDefinition, StateContractId};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(super) fn state_contract_id(app: &AppDefinition) -> StateContractId {
    #[derive(Serialize)]
    struct Contract<'a> {
        slug: &'a str,
        document: Value,
        theme: BTreeMap<String, String>,
    }
    let mut document = serde_json::to_value(&app.spec.schema).expect("accepted descriptor serializes");
    normalize_zero(&mut document);
    let contract =
        Contract { slug: &app.metadata.slug, document, theme: app.spec.theme_tokens().iter().cloned().collect() };
    let mut hash = Sha256::new();
    hash.update(b"pop.state-contract.v1\0");
    hash.update(serde_json::to_vec(&contract).expect("accepted state contract serializes"));
    StateContractId::from_bytes(hash.finalize().into())
}

// Parsed Node equality treats both floating zero signs as equal. Retain explicit
// absent/present options and every other parsed field in the serialized payload.
fn normalize_zero(value: &mut Value) {
    match value {
        Value::Number(number) if number.is_f64() && number.as_f64() == Some(0.0) => {
            *number = serde_json::Number::from_f64(0.0).expect("zero is finite");
        }
        Value::Object(fields) => {
            fields.values_mut().for_each(normalize_zero);
            // Feature unification may enable serde_json's insertion-ordered maps.
            // Contract identity must not depend on that build-graph choice.
            fields.sort_keys();
        }
        Value::Array(items) => items.iter_mut().for_each(normalize_zero),
        _ => {}
    }
}
