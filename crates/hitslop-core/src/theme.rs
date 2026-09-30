//! Theme rules shared by authoring validation and native writes. No document engine.
use crate::{encode, err, parse, Code, Result};
use std::collections::BTreeMap;

type Values = BTreeMap<String, String>;
fn check(values: &Values, known: &Values) -> Result<()> {
    for (name, value) in values {
        if !name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || name.starts_with(crate::wire::THEME_RESERVED_PREFIX)
            || !known.contains_key(name)
        {
            return Err(err(Code::InvalidKey, format!("Invalid theme token: {name}")));
        }
        if value.trim().is_empty() || value.encode_utf16().count() > crate::wire::THEME_VALUE_LIMIT
            || value.contains(['{', '}', ';'])
        {
            return Err(err(Code::OutOfRange, format!("Invalid theme value: {name}")));
        }
        for reference in value.split("var(").skip(1) {
            if let Some(reference) = reference.trim_start().strip_prefix("--slop-") {
                let end = reference.find(|c: char| !c.is_ascii_alphanumeric() && c != '-').unwrap_or(reference.len());
                if !known.contains_key(&reference[..end]) {
                    return Err(err(Code::InvalidKey, format!("Unknown theme reference in {name}")));
                }
            }
        }
    }
    Ok(())
}
pub fn validate_defaults(json: &str) -> Result<()> {
    let defaults: Values = parse(json)?;
    check(&defaults, &defaults)?;
    bound(&defaults)
}
fn bound(values: &Values) -> Result<()> {
    if encode(values)?.len() > crate::wire::THEME_LIMIT {
        return Err(err(Code::TooLarge, "Effective theme exceeds 64 KiB"));
    }
    Ok(())
}
/// A theme command against the stored overrides.
pub enum Change<'a> {
    Get,
    /// Merges these values (JSON) into the overrides.
    Set(&'a str),
    /// Removes one token's override, or all of them.
    Reset(Option<&'a str>),
}
/// Canonical JSON for the defaults, the overrides to store and the effective theme.
#[derive(Debug)]
pub struct ThemeState {
    pub defaults: String,
    pub overrides: String,
    pub effective: String,
}
/// The one theme rule set for page and CLI writes: unknown tokens, values, references
/// and the 64 KiB effective size are checked on every change. Reading never fails on
/// stored overrides; the browser ignores CSS it cannot parse.
pub fn apply(defaults: &str, overrides: &str, change: Change) -> Result<ThemeState> {
    let defaults: Values = parse(defaults)?;
    check(&defaults, &defaults)?;
    let mut overrides: Values = parse(overrides)?;
    let changed = !matches!(change, Change::Get);
    match change {
        Change::Get => {}
        Change::Set(values) => overrides.extend(parse::<Values>(values)?),
        Change::Reset(Some(token)) => {
            if !defaults.contains_key(token) {
                return Err(err(Code::InvalidKey, format!("Unknown theme token: {token}")));
            }
            overrides.remove(token);
        }
        Change::Reset(None) => overrides.clear(),
    }
    let mut effective = defaults.clone();
    effective.extend(overrides.iter().map(|(k, v)| (k.clone(), v.clone())));
    if changed {
        check(&overrides, &defaults)?;
        bound(&effective)?;
    }
    Ok(ThemeState { defaults: encode(&defaults)?, overrides: encode(&overrides)?, effective: encode(&effective)? })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn defaults_and_changes_share_names_values_references_and_size_rules() {
        let defaults = r#"{"accent":"red","paper":"var(--slop-accent)"}"#;
        validate_defaults(defaults).unwrap();
        for values in [json!({"window-radius":"1px"}), json!({"bad key":"red"}), json!({"accent":" "}), json!({"accent":"red;"}), json!({"accent":"var( --slop-missing)"}), json!({"accent":"😀".repeat(2049)})] {
            assert!(validate_defaults(&values.to_string()).is_err(), "{values}");
        }
        let large: Values = (0..20).map(|n| (format!("token-{n}"), "a".repeat(4096))).collect();
        assert_eq!(validate_defaults(&encode(&large).unwrap()).unwrap_err().code, Code::TooLarge);

        let set = apply(defaults, "{}", Change::Set(r#"{"accent":"blue"}"#)).unwrap();
        assert_eq!(set.overrides, r#"{"accent":"blue"}"#);
        assert_eq!(set.effective, r#"{"accent":"blue","paper":"var(--slop-accent)"}"#);
        assert_eq!(apply(defaults, "{}", Change::Set(r#"{"missing":"red"}"#)).unwrap_err().code, Code::InvalidKey);
        assert_eq!(apply(defaults, &set.overrides, Change::Reset(Some("missing"))).unwrap_err().code, Code::InvalidKey);
        assert_eq!(apply(defaults, &set.overrides, Change::Reset(Some("accent"))).unwrap().overrides, "{}");
        assert_eq!(apply(defaults, &set.overrides, Change::Reset(None)).unwrap().overrides, "{}");
        // The effective theme is bounded, not only the overrides.
        let wide = json!({"accent":"a".repeat(4096)}).to_string();
        let big: Values = (0..15).map(|n| (format!("t{n}"), "b".repeat(4096))).chain([("accent".to_owned(), "red".to_owned())]).collect();
        assert_eq!(apply(&encode(&big).unwrap(), "{}", Change::Set(&wide)).unwrap_err().code, Code::TooLarge);
        // Reading never fails on stored overrides.
        assert_eq!(apply(defaults, r#"{"accent":"red;"}"#, Change::Get).unwrap().overrides, r#"{"accent":"red;"}"#);
    }
}
