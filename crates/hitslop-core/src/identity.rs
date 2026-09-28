use super::*;
const ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}
pub(super) fn derived(internal: &str) -> String {
    let mut bits = (u128::from(fnv(internal)) << 64) | u128::from(fnv(&format!("hitslop:{internal}")));
    let mut out = String::from("x-");
    for _ in 0..24 { out.push(ALPHABET[(bits & 31) as usize] as char); bits >>= 5; }
    out
}
/// Pure projection. Stored identity registers are never repaired.
pub(super) fn rows(list: &LoroMovableList) -> Vec<Option<String>> {
    let entries: Vec<_> = (0..list.len()).map(|i| match list.get(i) {
        Some(ValueOrContainer::Container(Container::Map(map))) => {
            let stored = match map.get("$id") {
                Some(ValueOrContainer::Value(loro::LoroValue::String(s))) if valid_id(&s) => Some(s.to_string()),
                _ => None,
            };
            Some((map.id().to_string(), stored))
        }
        _ => None,
    }).collect();
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for (internal, stored) in entries.iter().flatten() {
        if let Some(id) = stored {
            let owner = owners.entry(id.clone()).or_insert_with(|| internal.clone());
            if internal < owner { *owner = internal.clone(); }
        }
    }
    let mut taken: BTreeSet<String> = owners.keys().cloned().collect();
    entries.into_iter().map(|entry| entry.map(|(internal, stored)| {
        if let Some(id) = stored {
            if owners.get(&id) == Some(&internal) { return id; }
        }
        let mut id = derived(&internal);
        let mut salt = 1;
        while taken.contains(&id) { id = derived(&format!("{internal}#{salt}")); salt += 1; }
        taken.insert(id.clone());
        id
    })).collect()
}
#[test]
fn frozen_identity_vectors() {
    assert_eq!(derived("cid:7@12345:Map"), "x-r01fjb7pch2ptdbhx3heg86d");
    assert_eq!(derived("2@99"), "x-8ytk4r1fvcw0kgf6n48rg87f");
}
