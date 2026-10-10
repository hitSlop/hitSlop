use super::*;
#[cfg(feature = "storage")]
#[cfg(not(target_arch = "wasm32"))]
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}
/// A valid row ID derived from `seed`: the same seed always gives the same ID, so packing
/// an app twice writes the same template.
#[cfg(feature = "storage")]
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn derived(seed: &str) -> String {
    let mut bits = (u128::from(fnv(seed)) << 64) | u128::from(fnv(&format!("hitslop:{seed}")));
    let mut out = String::from("x-");
    for _ in 0..24 {
        out.push(wire::ID_ALPHABET[(bits & 31) as usize] as char);
        bits >>= 5;
    }
    out
}
pub(super) fn application_id() -> String {
    let mut bytes = [0u8; 16];
    random(&mut bytes);
    let mut buffer = 0u32;
    let mut bits = 0;
    let mut out = String::new();
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(wire::ID_ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    out.push(wire::ID_ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    out
}

#[cfg(all(test, feature = "storage", not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn derived_ids_are_stable() {
        assert_eq!(derived("cid:7@12345:Map"), "x-r01fjb7pch2ptdbhx3heg86d");
        assert_eq!(derived("2@99"), "x-8ytk4r1fvcw0kgf6n48rg87f");
    }
}
