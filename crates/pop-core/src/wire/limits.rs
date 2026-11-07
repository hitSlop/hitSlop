pub use pop_contracts::wire::limits::*;
#[path = "acceptance.rs"]
mod acceptance;
impl crate::app::AppDefinition {
    /// The app reader's stored-data bounds, including the resources and state it owns.
    pub fn open_limits() -> Vec<Limit> {
        open_limits()
    }

    /// Checks a release's limits table without passing u64 values through JavaScript numbers.
    pub fn check_recorded_limits(input: &str) -> crate::Result<()> {
        let recorded: Vec<Limit> = serde_json::from_str(input)
            .map_err(|error| crate::err(crate::Code::InvalidBytes, format!("Missing or invalid limits: {error}")))?;
        if recorded.is_empty() {
            return Err(crate::err(crate::Code::InvalidBytes, "Empty limits table"));
        }
        let current = open_limits();
        let mut names = std::collections::HashSet::new();
        for old in recorded {
            if !names.insert(old.name.clone()) {
                return Err(crate::err(crate::Code::InvalidBytes, format!("Duplicate limit {}", old.name)));
            }
            match current.iter().find(|limit| limit.name == old.name) {
                Some(now) if !now.accepts_recorded(&old) => {
                    return Err(crate::err(
                        crate::Code::InvalidBytes,
                        format!(
                            "{} became stricter: recorded {:?} {}, current {:?} {}",
                            old.name, old.kind, old.value, now.kind, now.value
                        ),
                    ));
                }
                None if !RETIRED_LIMITS.contains(&old.name.as_str()) => {
                    return Err(crate::err(
                        crate::Code::InvalidBytes,
                        format!("Recorded limit {} disappeared", old.name),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}
