use super::*;
pub(super) const ALPHABET: &[u8] = crate::wire::ID_ALPHABET;
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}
pub(super) fn derived(internal: &str) -> String {
    let mut bits = (u128::from(fnv(internal)) << 64) | u128::from(fnv(&format!("hitslop:{internal}")));
    let mut out = String::from("x-");
    for _ in 0..24 { out.push(ALPHABET[(bits & 31) as usize] as char); bits >>= 5; }
    out
}
/// A row's stored `$id`, when it is a valid application ID.
pub(super) fn stored_id(map: &LoroMap) -> Option<String> {
    match map.get("$id") {
        Some(ValueOrContainer::Value(loro::LoroValue::String(s))) if valid_id(&s) => Some(s.to_string()),
        _ => None,
    }
}
fn entries(list: &LoroMovableList) -> Vec<Option<(ContainerID, Option<String>)>> {
    let mut entries = Vec::with_capacity(list.len());
    list.for_each(|row| entries.push(match row {
        ValueOrContainer::Container(Container::Map(map)) => Some((map.id(), stored_id(&map))),
        _ => None,
    }));
    entries
}
/// The stored IDs when every row is a map with its own unique valid ID (the common case,
/// where stored and effective IDs agree).
fn clean(entries: &[Option<(ContainerID, Option<String>)>]) -> Option<Vec<String>> {
    let mut unique = HashSet::with_capacity(entries.len());
    entries.iter().map(|entry| match entry {
        Some((_, Some(id))) if unique.insert(id.as_str()) => Some(id.clone()),
        _ => None,
    }).collect()
}
pub(super) fn clean_rows(list: &LoroMovableList) -> Option<Vec<String>> {
    clean(&entries(list))
}
/// Pure projection. Stored identity registers are never repaired.
pub(super) fn rows(list: &LoroMovableList) -> Vec<Option<String>> {
    let entries = entries(list);
    if let Some(ids) = clean(&entries) {
        return ids.into_iter().map(Some).collect();
    }
    let entries: Vec<_> = entries.into_iter().map(|entry| entry.map(|(cid, stored)| (cid.to_string(), stored))).collect();
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
