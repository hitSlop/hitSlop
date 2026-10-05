use super::*;
#[cfg(feature = "storage")]
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}
/// A valid row ID derived from `seed`: the same seed always gives the same ID, so packing
/// an app twice writes the same template.
#[cfg(feature = "storage")]
pub(super) fn derived(seed: &str) -> String {
    let mut bits = (u128::from(fnv(seed)) << 64) | u128::from(fnv(&format!("hitslop:{seed}")));
    let mut out = String::from("x-");
    for _ in 0..24 { out.push(wire::ID_ALPHABET[(bits & 31) as usize] as char); bits >>= 5; }
    out
}
/// A row's stored `$id`, when it is a valid application ID.
pub(super) fn stored_id(map: &LoroMap) -> Option<String> {
    match map.get("$id") {
        Some(ValueOrContainer::Value(loro::LoroValue::String(s))) if valid_id(&s) => Some(s.to_string()),
        _ => None,
    }
}
/// Every row's `$id`, in order. Each row stores its own unique ID: the open-time check and
/// every write keep it so.
pub(super) fn rows(list: &LoroMovableList) -> Vec<String> {
    let mut ids = Vec::with_capacity(list.len());
    list.for_each(|row| ids.push(match row {
        ValueOrContainer::Container(Container::Map(map)) => stored_id(&map).unwrap_or_default(),
        _ => String::new(),
    }));
    ids
}
#[cfg(feature = "storage")]
#[test]
fn derived_ids_are_stable() {
    assert_eq!(derived("cid:7@12345:Map"), "x-r01fjb7pch2ptdbhx3heg86d");
    assert_eq!(derived("2@99"), "x-8ytk4r1fvcw0kgf6n48rg87f");
}
