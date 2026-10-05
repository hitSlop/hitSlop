//! Theme rules shared by authoring validation and native writes. A theme is a palette:
//! the colors an app declares, which the person may override. No document engine.
use crate::wire::{ThemeFile, THEME_FILE_LIMIT, THEME_LIMIT, THEME_NAME_LIMIT, THEME_RESERVED_PREFIX, THEME_TOKENS};
use crate::{encode, err, parse, Code, Result};
use std::collections::BTreeMap;

type Values = BTreeMap<String, String>;
fn valid_name(name: &str) -> bool {
    name.len() <= THEME_NAME_LIMIT
        && name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !name.starts_with(THEME_RESERVED_PREFIX)
}
/// Lowercase `#rrggbb` or `#rrggbbaa`. An opaque color omits `ff`, so two values name the
/// same color only when they are the same string.
fn valid_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else { return false };
    matches!(hex.len(), 6 | 8)
        && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        && !(hex.len() == 8 && hex.ends_with("ff"))
}
fn check(values: &Values, known: &Values) -> Result<()> {
    if values.len() > THEME_TOKENS {
        return Err(err(Code::TooLarge, format!("A theme has at most {THEME_TOKENS} colors")));
    }
    for (name, value) in values {
        if !valid_name(name) || !known.contains_key(name) {
            return Err(err(Code::InvalidKey, format!("Invalid theme token: {name}")));
        }
        if !valid_color(value) {
            return Err(err(Code::OutOfRange, format!("Theme color {name} must be lowercase #rrggbb or #rrggbbaa")));
        }
    }
    Ok(())
}
fn bound(values: &Values) -> Result<()> {
    if encode(values).len() > THEME_LIMIT {
        return Err(err(Code::TooLarge, "Effective theme exceeds 64 KiB"));
    }
    Ok(())
}

/// A JSON object's entries in the order written, refusing repeated names.
struct Ordered(Vec<(String, String)>);
impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Entries;
        impl<'de> serde::de::Visitor<'de> for Entries {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object of theme colors")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> std::result::Result<Ordered, A::Error> {
                let mut entries = Vec::<(String, String)>::new();
                while let Some((name, value)) = map.next_entry::<String, String>()? {
                    if entries.iter().any(|(seen, _)| *seen == name) {
                        return Err(serde::de::Error::custom(format!("Repeated theme token: {name}")));
                    }
                    entries.push((name, value));
                }
                Ok(Ordered(entries))
            }
        }
        d.deserialize_map(Entries)
    }
}
/// An app's declared colors (its `app` row's theme), in the order the author wrote them.
/// Refused by size before parsing, so the repeated-name check stays small.
pub fn validate_defaults(json: &str) -> Result<Vec<(String, String)>> {
    if json.len() > THEME_LIMIT {
        return Err(err(Code::TooLarge, "Theme defaults exceed 64 KiB"));
    }
    let Ordered(tokens) = parse(json)?;
    let defaults: Values = tokens.iter().cloned().collect();
    check(&defaults, &defaults)?;
    bound(&defaults)?;
    Ok(tokens)
}

/// A theme command.
pub enum Change<'a> {
    Get,
    /// Merges these values (JSON) into the overrides.
    Set(&'a str),
    /// Removes one token's override, or all of them.
    Reset(Option<&'a str>),
    /// Replaces the overrides with a theme file, which must be for this palette's template.
    Import(&'a str),
}
/// Canonical JSON for the defaults, the overrides and the effective theme.
#[derive(Debug)]
pub struct ThemeState {
    pub defaults: String,
    pub overrides: String,
    pub effective: String,
}

/// A document's palette: its template's declared colors and the owner's overrides. The
/// template's slug names it in theme files.
#[derive(Clone, Debug)]
pub(crate) struct Theme {
    template: String,
    defaults: Values,
    overrides: Values,
}
impl Theme {
    /// The palette over `defaults`, which `validate_defaults` returned. Stored overrides are
    /// kept as read: reading never fails on them.
    pub fn new(template: &str, defaults: &[(String, String)], overrides: &str) -> Result<Self> {
        Ok(Self { template: template.into(), defaults: defaults.iter().cloned().collect(), overrides: parse(overrides)? })
    }
    /// The defaults with `overrides` applied.
    fn over(&self, overrides: &Values) -> Values {
        let mut effective = self.defaults.clone();
        effective.extend(overrides.iter().map(|(k, v)| (k.clone(), v.clone())));
        effective
    }
    fn effective(&self) -> Values {
        self.over(&self.overrides)
    }
    pub fn state(&self) -> ThemeState {
        ThemeState { defaults: encode(&self.defaults), overrides: encode(&self.overrides), effective: encode(&self.effective()) }
    }
    pub fn overrides(&self) -> String {
        encode(&self.overrides)
    }
    /// Applies a change under the palette rules; the theme is unchanged unless the whole
    /// result is valid. Overrides equal to their default are dropped, so resetting a token
    /// and setting its default are the same change. Returns whether the overrides changed.
    pub fn change(&mut self, change: Change) -> Result<bool> {
        let mut next = match change {
            Change::Get => return Ok(false),
            Change::Set(values) => {
                let mut next = self.overrides.clone();
                next.extend(parse::<Values>(values)?);
                next
            }
            Change::Reset(Some(token)) => {
                if !self.defaults.contains_key(token) {
                    return Err(err(Code::InvalidKey, format!("Unknown theme token: {token}")));
                }
                let mut next = self.overrides.clone();
                next.remove(token);
                next
            }
            Change::Reset(None) => Values::new(),
            Change::Import(file) => read_file(&self.template, file)?,
        };
        check(&next, &self.defaults)?;
        next.retain(|name, value| self.defaults.get(name) != Some(value));
        bound(&self.over(&next))?;
        if next == self.overrides {
            return Ok(false);
        }
        self.overrides = next;
        Ok(true)
    }
    /// The full effective palette as a theme file for this template: canonical JSON and a
    /// final newline, the bytes every export writes.
    pub fn export(&self) -> String {
        encode(&ThemeFile { template: self.template.clone(), values: self.effective() }) + "\n"
    }
}
/// A theme file's palette, refused whole unless it is a theme file for `template`.
fn read_file(template: &str, file: &str) -> Result<Values> {
    if file.len() > THEME_FILE_LIMIT {
        return Err(err(Code::TooLarge, "Theme file is too large"));
    }
    let file: ThemeFile =
        serde_json::from_str(file).map_err(|_| err(Code::InvalidRequest, "Not a hitSlop theme file"))?;
    if file.template != template {
        return Err(err(Code::InvalidRequest, format!("This theme is for {}", file.template)));
    }
    Ok(file.values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const DEFAULTS: &str = r##"{"paper":"#f4efe6","accent":"#a43d59","ink":"#2a2522"}"##;
    fn theme() -> Theme {
        Theme::new("kanban-board", &validate_defaults(DEFAULTS).unwrap(), "{}").unwrap()
    }
    fn code(result: Result<impl std::fmt::Debug>) -> Code {
        result.unwrap_err().code
    }

    #[test]
    fn defaults_are_a_palette_in_authored_order() {
        let tokens = validate_defaults(DEFAULTS).unwrap();
        assert_eq!(tokens.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["paper", "accent", "ink"]);
        validate_defaults(r##"{"overlay":"#0a0c10d9","clear":"#00000000"}"##).unwrap();
        for values in [
            json!({"font":"\"Avenir Next\", sans-serif"}),
            json!({"accent":"red"}),
            json!({"accent":"#fff"}),
            json!({"accent":"#FFFFFF"}),
            json!({"accent":"#aabbccff"}),
            json!({"accent":"rgba(1, 2, 3, 0.5)"}),
            json!({"rule":"color-mix(in srgb, var(--slop-ink) 14%, transparent)"}),
            json!({"accent":" #ffffff"}),
            json!({"bad key":"#ffffff"}),
            json!({"1accent":"#ffffff"}),
            json!({"window-radius":"#ffffff"}),
            json!({"a".repeat(65): "#ffffff"}),
        ] {
            assert!(validate_defaults(&values.to_string()).is_err(), "{values}");
        }
        validate_defaults(&json!({"a".repeat(64): "#ffffff"}).to_string()).unwrap();
        assert!(validate_defaults(r##"{"ink":"#000000","ink":"#111111"}"##).is_err());
        let many: Values = (0..=THEME_TOKENS).map(|n| (format!("t{n}"), "#000000".into())).collect();
        assert_eq!(code(validate_defaults(&encode(&many))), Code::TooLarge);
        // Refused by its size before it is parsed.
        assert_eq!(code(validate_defaults(&format!("{{{}}}", " ".repeat(THEME_LIMIT)))), Code::TooLarge);
    }

    #[test]
    fn changes_keep_only_colors_that_differ_from_the_defaults() {
        let mut theme = theme();
        assert!(theme.change(Change::Set(r##"{"accent":"#123456"}"##)).unwrap());
        assert_eq!(theme.overrides(), r##"{"accent":"#123456"}"##);
        assert!(theme.state().effective.contains(r##""accent":"#123456""##));
        // Setting a default is a reset, and a repeated change changes nothing.
        assert!(!theme.change(Change::Set(r##"{"accent":"#123456"}"##)).unwrap());
        assert!(theme.change(Change::Set(r##"{"accent":"#a43d59"}"##)).unwrap());
        assert_eq!(theme.overrides(), "{}");
        theme.change(Change::Set(r##"{"accent":"#123456","ink":"#000000"}"##)).unwrap();
        assert!(theme.change(Change::Reset(Some("accent"))).unwrap());
        assert_eq!(theme.overrides(), r##"{"ink":"#000000"}"##);
        assert!(theme.change(Change::Reset(None)).unwrap());
        assert_eq!(theme.overrides(), "{}");
        assert!(!theme.change(Change::Reset(None)).unwrap());
    }

    #[test]
    fn a_refused_change_leaves_the_theme_unchanged() {
        let mut theme = theme();
        theme.change(Change::Set(r##"{"ink":"#000000"}"##)).unwrap();
        for values in [r##"{"missing":"#000000"}"##, r##"{"ink":"black"}"##, r##"{"ink":"#000000ff"}"##, "[]"] {
            assert!(theme.change(Change::Set(values)).is_err(), "{values}");
        }
        assert_eq!(code(theme.change(Change::Reset(Some("missing")))), Code::InvalidKey);
        assert_eq!(theme.overrides(), r##"{"ink":"#000000"}"##);
        // Reading never fails on stored overrides.
        let stored = Theme::new("kanban-board", &validate_defaults(DEFAULTS).unwrap(), r##"{"ink":"black"}"##).unwrap();
        assert_eq!(stored.overrides(), r##"{"ink":"black"}"##);
    }

    #[test]
    fn a_theme_file_round_trips_and_imports_whole_or_not_at_all() {
        let mut source = theme();
        source.change(Change::Set(r##"{"accent":"#123456","paper":"#ffffff"}"##)).unwrap();
        let file = source.export();
        assert_eq!(file, source.export());
        let parsed: serde_json::Value = serde_json::from_str(&file).unwrap();
        assert_eq!(parsed["template"], "kanban-board");
        assert_eq!(parsed["values"].as_object().unwrap().len(), 3, "the full palette");

        let mut target = theme();
        target.change(Change::Set(r##"{"ink":"#000000"}"##)).unwrap();
        assert!(target.change(Change::Import(&file)).unwrap());
        // Only differences are kept; tokens the file leaves at their defaults are reset.
        assert_eq!(target.overrides(), r##"{"accent":"#123456","paper":"#ffffff"}"##);
        assert_eq!(target.state().effective, source.state().effective);

        let partial = json!({"template":"kanban-board","values":{"ink":"#111111"}}).to_string();
        target.change(Change::Import(&partial)).unwrap();
        assert_eq!(target.overrides(), r##"{"ink":"#111111"}"##);

        let before = target.overrides();
        let elsewhere = json!({"template":"habit-heatmap","values":{"ink":"#111111"}}).to_string();
        let refused = [
            (elsewhere.clone(), Code::InvalidRequest),
            (json!({"template":"kanban-board","values":{"missing":"#000000"}}).to_string(), Code::InvalidKey),
            (json!({"template":"kanban-board","values":{"ink":"black"}}).to_string(), Code::OutOfRange),
            (json!({"template":"kanban-board","values":{},"extra":1}).to_string(), Code::InvalidRequest),
            ("not json".to_owned(), Code::InvalidRequest),
            (" ".repeat(THEME_FILE_LIMIT + 1), Code::TooLarge),
        ];
        for (file, expected) in refused {
            assert_eq!(code(target.change(Change::Import(&file))), expected, "{file:.80}");
        }
        assert_eq!(target.overrides(), before);
        let message = target.change(Change::Import(&elsewhere)).unwrap_err().message;
        assert_eq!(message, "This theme is for habit-heatmap");
    }

    #[test]
    fn the_effective_theme_and_its_file_stay_within_their_limits() {
        // The largest valid palette exports to a file that imports again.
        let defaults: Values = (0..THEME_TOKENS).map(|n| (format!("t{n:a<63}"), "#00000000".to_owned())).collect();
        let defaults = encode(&defaults);
        assert!(defaults.len() <= THEME_LIMIT);
        let mut theme = Theme::new("a-template-with-a-long-name-that-uses-all-sixty-four-characters-x", &validate_defaults(&defaults).unwrap(), "{}").unwrap();
        let file = theme.export();
        assert!(file.len() <= THEME_FILE_LIMIT, "{}", file.len());
        theme.change(Change::Import(&file)).unwrap();
    }
}
