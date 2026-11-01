//! Deterministic native template checkpoints.
use super::*;
use crate::identity;

/// The peer a template's initial operations belong to (`Document::initial_checkpoint`).
const TEMPLATE_PEER: u64 = 1;
/// Gives every row in `value` that has no `$id` one derived from its place (`at`).
fn name_rows(node: &Node, value: &mut Value, at: &str) {
    match (node, value) {
        (Node::Optional { inner }, value) => name_rows(inner, value, at),
        (Node::Object { properties }, Value::Object(fields)) => {
            for (key, child) in properties {
                if let Some(field) = fields.get_mut(key) {
                    name_rows(child, field, &format!("{at}/{key}"));
                }
            }
        }
        (Node::Record { value: entry }, Value::Object(entries)) => {
            for (key, field) in entries.iter_mut() {
                name_rows(entry, field, &format!("{at}/{key}"));
            }
        }
        (Node::List { item }, Value::Array(rows)) if matches!(**item, Node::Object { .. }) => {
            for (index, row) in rows.iter_mut().enumerate() {
                let place = format!("{at}/{index}");
                if let Value::Object(fields) = row {
                    fields
                        .entry("$id")
                        .or_insert_with(|| Value::String(identity::derived(&format!("initial:{place}"))));
                }
                name_rows(item, row, &place);
            }
        }
        _ => {}
    }
}

/// Semantic seed equality: object order and integral number spelling may differ,
/// but converting an integer to a double must not silently round its value.
fn same_seed(a: &Value, b: &Value) -> bool {
    fn integer_float(integer: &serde_json::Number, float: f64) -> bool {
        if float.fract() != 0.0 {
            return false;
        }
        if let Some(n) = integer.as_i64() {
            float >= i64::MIN as f64 && float < i64::MAX as f64 && float as i64 == n
        } else if let Some(n) = integer.as_u64() {
            float >= 0.0 && float < u64::MAX as f64 && float as u64 == n
        } else {
            false
        }
    }
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if a != b => match (a.is_f64(), b.is_f64()) {
            (false, true) => integer_float(a, b.as_f64().expect("float")),
            (true, false) => integer_float(b, a.as_f64().expect("float")),
            _ => false,
        },
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_seed(a, b)),
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(key, a)| b.get(key).is_some_and(|b| same_seed(a, b)))
        }
        _ => a == b,
    }
}

impl Document {
    /// A template's initial state, as the checkpoint every document of it starts from. The
    /// same app always packs the same bytes, so a rebuild reproduces its template: rows
    /// without an `$id` get one derived from their place, and the operations belong to a
    /// fixed peer. A document that opens it edits as a peer of its own.
    pub(crate) fn initial_checkpoint(app: &AppSpec, initial: &str) -> Result<Vec<u8>> {
        let mut initial: Value = parse(initial)?;
        app.schema.fill_defaults(&mut initial);
        app.schema.validate(&initial, false)?;
        name_rows(&app.schema, &mut initial, "");
        let doc = LoroDoc::new();
        doc.set_peer_id(TEMPLATE_PEER).map_err(engine)?;
        let checkpoint = filled(doc, &app.schema, &initial)?.export(ExportMode::Snapshot).map_err(engine)?;
        let reopened = Self::open(app, &checkpoint, &[])?;
        let saved: Value = parse(&reopened.value())?;
        if !same_seed(&initial, &saved) {
            return Err(err(Code::InvalidRequest, "The initial checkpoint would change the declared values"));
        }
        Ok(checkpoint)
    }
}
