//! Theme rules shared by authoring validation and native writes. A theme is a palette:
//! the colors an app declares, with overrides in the document's own Loro history.
use crate::wire::{THEME_FILE_LIMIT, THEME_LIMIT, THEME_NAME_LIMIT, THEME_RESERVED_PREFIX, THEME_TOKENS, ThemeFile};
use crate::{Code, Result, encode, err, parse};
use loro::{LoroMap, LoroValue, ValueOrContainer};
use std::collections::{BTreeMap, HashSet};

pub(crate) type Values = BTreeMap<String, String>;
pub(crate) const ROOT: &str = "theme";
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
                let (mut entries, mut seen) = (Vec::<(String, String)>::new(), HashSet::new());
                while let Some((name, value)) = map.next_entry::<String, String>()? {
                    if !seen.insert(name.clone()) {
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
    validate_tokens(&tokens)?;
    Ok(tokens)
}
/// Typed declarations and stored definitions use the same palette rules as writes.
pub(crate) fn validate_tokens(tokens: &[(String, String)]) -> Result<()> {
    let defaults: Values = tokens.iter().cloned().collect();
    if defaults.len() != tokens.len() {
        return Err(err(Code::InvalidKey, "Repeated theme token"));
    }
    check(&defaults, &defaults)?;
    bound(&defaults)
}

/// The defaults, overrides and effective palette, already checked by the core.
#[derive(Debug)]
pub struct ThemeState {
    pub defaults: BTreeMap<String, String>,
    pub overrides: BTreeMap<String, String>,
    pub effective: BTreeMap<String, String>,
}

/// An app's declared palette, in the order the author wrote it. The template's slug names
/// it in theme files.
#[derive(Clone, Debug)]
pub(crate) struct Theme {
    template: String,
    tokens: Vec<(String, String)>,
    defaults: Values,
}
impl Theme {
    /// The palette over tokens already checked by `validate_defaults`.
    pub fn new(template: &str, tokens: Vec<(String, String)>) -> Self {
        Self { template: template.into(), defaults: tokens.iter().cloned().collect(), tokens }
    }
    pub fn template(&self) -> &str {
        &self.template
    }
    pub fn tokens(&self) -> &[(String, String)] {
        &self.tokens
    }
    /// The defaults with `overrides` applied.
    fn over(&self, overrides: &Values) -> Values {
        let mut effective = self.defaults.clone();
        effective.extend(overrides.iter().map(|(k, v)| (k.clone(), v.clone())));
        effective
    }
    pub fn effective(&self, map: &LoroMap) -> Result<Values> {
        Ok(self.over(&overrides(map)?))
    }
    pub fn state(&self, map: &LoroMap) -> Result<ThemeState> {
        let overrides = overrides(map)?;
        Ok(ThemeState { defaults: self.defaults.clone(), effective: self.over(&overrides), overrides })
    }
    /// Sets each listed color, or with `None` returns it to the template's; `replace`
    /// returns every unlisted color to the template's too.
    pub fn set(&self, map: &LoroMap, values: &BTreeMap<String, Option<String>>, replace: bool) -> Result<()> {
        let current = overrides(map)?;
        let mut next = if replace { Values::new() } else { current.clone() };
        for (name, value) in values {
            if !self.defaults.contains_key(name) {
                return Err(err(Code::InvalidKey, format!("Unknown theme token: {name}")));
            }
            match value {
                Some(value) => next.insert(name.clone(), value.clone()),
                None => next.remove(name),
            };
        }
        self.write(map, &current, next)
    }
    /// Replaces the overrides with a theme file, which must be for this palette's template.
    pub fn import(&self, map: &LoroMap, file: &str) -> Result<()> {
        self.write(map, &overrides(map)?, read_file(&self.template, file)?)
    }
    /// Writes `next` under the palette rules; the theme is unchanged unless the whole
    /// result is valid. Overrides equal to their default are dropped, so resetting a token
    /// and setting its default are the same change.
    fn write(&self, map: &LoroMap, current: &Values, mut next: Values) -> Result<()> {
        check(&next, &self.defaults)?;
        next.retain(|name, value| self.defaults.get(name) != Some(value));
        bound(&self.over(&next))?;
        for name in current.keys().filter(|name| !next.contains_key(*name)) {
            map.delete(name).map_err(crate::engine)?;
        }
        for (name, value) in &next {
            if current.get(name) != Some(value) {
                map.insert(name, value.as_str()).map_err(crate::engine)?;
            }
        }
        Ok(())
    }
    /// Refuses saved overrides no accepted change writes: an undeclared token, an invalid
    /// color, or a color equal to its default.
    pub fn check_stored(&self, map: &LoroMap) -> Result<()> {
        let invalid =
            || err(Code::InvalidBytes, "Saved theme does not match the app's palette; keep the file for recovery");
        let overrides = overrides(map).map_err(|_| invalid())?;
        check(&overrides, &self.defaults).map_err(|_| invalid())?;
        if overrides.iter().any(|(name, value)| self.defaults.get(name) == Some(value)) {
            return Err(invalid());
        }
        Ok(())
    }
    /// The full effective palette as a theme file for this template: canonical JSON and a
    /// final newline, the bytes every export writes.
    pub fn export(&self, map: &LoroMap) -> Result<String> {
        Ok(encode(&ThemeFile { template: self.template.clone(), values: self.effective(map)? }) + "\n")
    }
}
/// The saved overrides. Opening checks them (`Theme::check_stored`) and every write
/// validates the complete proposed palette before touching this map.
fn overrides(map: &LoroMap) -> Result<Values> {
    let mut values = Values::new();
    let mut invalid = false;
    map.for_each(|key, value| match value {
        ValueOrContainer::Value(LoroValue::String(value)) => {
            values.insert(key.to_owned(), value.to_string());
        }
        _ => invalid = true,
    });
    if invalid {
        return Err(err(Code::InvalidBytes, "Theme overrides must contain colors"));
    }
    Ok(values)
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
    use crate::{AppSpec, Document, Origin};
    use serde_json::json;
    const DEFAULTS: &str = r##"{"paper":"#f4efe6","accent":"#a43d59","ink":"#2a2522"}"##;
    const EMPTY: &str = r#"{"kind":"object","properties":{}}"#;
    struct Palette(Document);
    impl Palette {
        fn new(template: &str, defaults: &str) -> Self {
            Self(Document::create(&AppSpec::new(EMPTY, template, defaults).unwrap(), "{}").unwrap())
        }
        /// Applies one palette intent (JSON text); whether it changed anything.
        fn apply(&mut self, intent: &str) -> Result<bool> {
            self.0
                .apply_batch(crate::Batch::decode(&format!(r#"{{"intents":[{intent}]}}"#))?, Origin::Window)
                .map(|a| a.publication.is_some())
        }
        fn set(&mut self, values: &str) -> Result<bool> {
            self.apply(&format!(r#"{{"type":"setTheme","values":{values}}}"#))
        }
        fn reset(&mut self, token: Option<&str>) -> Result<bool> {
            match token {
                Some(token) => self.set(&json!({ token: null }).to_string()),
                None => self.apply(r#"{"type":"setTheme","values":{},"replace":true}"#),
            }
        }
        fn import(&mut self, file: &str) -> Result<bool> {
            self.apply(&json!({"type":"importTheme","file":file}).to_string())
        }
        fn state(&self) -> ThemeState {
            self.0.theme_state().unwrap()
        }
        fn overrides(&self) -> String {
            encode(&self.state().overrides)
        }
        fn export(&self) -> String {
            self.0.export_theme().unwrap()
        }
    }
    fn theme() -> Palette {
        Palette::new("kanban-board", DEFAULTS)
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
        assert!(theme.set(r##"{"accent":"#123456"}"##).unwrap());
        assert_eq!(theme.overrides(), r##"{"accent":"#123456"}"##);
        assert!(theme.state().effective.get("accent").is_some_and(|v| v == "#123456"));
        // Setting a default is a reset, and a repeated change changes nothing.
        assert!(!theme.set(r##"{"accent":"#123456"}"##).unwrap());
        assert!(theme.set(r##"{"accent":"#a43d59"}"##).unwrap());
        assert_eq!(theme.overrides(), "{}");
        theme.set(r##"{"accent":"#123456","ink":"#000000"}"##).unwrap();
        assert!(theme.reset(Some("accent")).unwrap());
        assert_eq!(theme.overrides(), r##"{"ink":"#000000"}"##);
        // `replace` returns every unlisted color to the template's.
        theme.set(r##"{"accent":"#123456"}"##).unwrap();
        assert!(theme.apply(r##"{"type":"setTheme","values":{"paper":"#ffffff"},"replace":true}"##).unwrap());
        assert_eq!(theme.overrides(), r##"{"paper":"#ffffff"}"##);
        assert!(theme.reset(None).unwrap());
        assert_eq!(theme.overrides(), "{}");
        assert!(!theme.reset(None).unwrap());
    }

    #[test]
    fn a_refused_change_leaves_the_theme_unchanged() {
        let mut theme = theme();
        theme.set(r##"{"ink":"#000000"}"##).unwrap();
        for values in [r##"{"missing":"#000000"}"##, r##"{"ink":"black"}"##, r##"{"ink":"#000000ff"}"##, "[]"] {
            assert!(theme.set(values).is_err(), "{values}");
        }
        assert_eq!(code(theme.reset(Some("missing"))), Code::InvalidKey);
        assert_eq!(theme.overrides(), r##"{"ink":"#000000"}"##);
    }

    #[test]
    fn the_page_cannot_change_the_palette() {
        let mut theme = theme();
        let batch = r##"{"intents":[{"type":"setTheme","values":{"ink":"#000000"}}]}"##;
        assert_eq!(code(theme.0.apply_batch(crate::Batch::decode(batch).unwrap(), Origin::Page)), Code::InvalidRequest);
        theme.0.apply_batch(crate::Batch::decode(batch).unwrap(), Origin::Agent).unwrap();
        assert_eq!(theme.overrides(), r##"{"ink":"#000000"}"##);
    }

    // Failure: opening preserved overrides that no accepted change writes, so the palette
    // read differently from what every write enforces. Oracle: the open-time refusal.
    #[test]
    fn saved_overrides_outside_the_palette_are_refused_at_open() {
        let app = AppSpec::new(EMPTY, "kanban-board", DEFAULTS).unwrap();
        for (name, value) in [("ink", "black"), ("missing", "#000000"), ("ink", "#2a2522")] {
            let theme = theme();
            theme.0.doc.get_map(ROOT).insert(name, value).unwrap();
            theme.0.doc.commit();
            let saved = theme.0.checkpoint().unwrap();
            assert_eq!(code(Document::open(&app, &saved, &[]).map(|_| ())), Code::InvalidBytes, "{name}: {value}");
        }
    }

    #[test]
    fn a_theme_file_round_trips_and_imports_whole_or_not_at_all() {
        let mut source = theme();
        source.set(r##"{"accent":"#123456","paper":"#ffffff"}"##).unwrap();
        let file = source.export();
        assert_eq!(file, source.export());
        let parsed: serde_json::Value = serde_json::from_str(&file).unwrap();
        assert_eq!(parsed["template"], "kanban-board");
        assert_eq!(parsed["values"].as_object().unwrap().len(), 3, "the full palette");

        let mut target = theme();
        target.set(r##"{"ink":"#000000"}"##).unwrap();
        assert!(target.import(&file).unwrap());
        // Only differences are kept; tokens the file leaves at their defaults are reset.
        assert_eq!(target.overrides(), r##"{"accent":"#123456","paper":"#ffffff"}"##);
        assert_eq!(target.state().effective, source.state().effective);

        let partial = json!({"template":"kanban-board","values":{"ink":"#111111"}}).to_string();
        target.import(&partial).unwrap();
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
            assert_eq!(code(target.import(&file)), expected, "{file:.80}");
        }
        assert_eq!(target.overrides(), before);
        let message = target.import(&elsewhere).unwrap_err().message;
        assert_eq!(message, "This theme is for habit-heatmap");
    }

    #[test]
    fn the_effective_theme_and_its_file_stay_within_their_limits() {
        // The largest valid palette exports to a file that imports again.
        let defaults: Values = (0..THEME_TOKENS).map(|n| (format!("t{n:a<63}"), "#00000000".to_owned())).collect();
        let defaults = encode(&defaults);
        assert!(defaults.len() <= THEME_LIMIT);
        let mut theme = Palette::new("a-template-with-a-long-name-that-uses-all-sixty-four-characters-x", &defaults);
        let file = theme.export();
        assert!(file.len() <= THEME_FILE_LIMIT, "{}", file.len());
        theme.import(&file).unwrap();
    }
}
