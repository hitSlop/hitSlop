use super::*;
use pop_contracts::fnv;
/// A valid row ID derived from `seed`: the same seed always gives the same ID, so packing
/// an app twice writes the same template.
pub(super) fn derived(seed: &str) -> String {
    let mut bits = (u128::from(fnv(seed)) << 64) | u128::from(fnv(&format!("pop:{seed}")));
    let mut out = String::from("x-");
    for _ in 0..24 {
        out.push(wire::ID_ALPHABET[(bits & 31) as usize] as char);
        bits >>= 5;
    }
    out
}
pub(super) fn application_id() -> String {
    let mut bytes = [0u8; 16];
    random(&mut bytes).expect("random bytes");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_ids_are_stable() {
        assert_eq!(derived("cid:7@12345:Map"), "x-2xejnqmpmn96gdbhx3heg86d");
        assert_eq!(derived("2@99"), "x-2r0xn5wy6pymmgf6n48rg87f");
    }
}
