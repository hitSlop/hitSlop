use pop_contracts::wire::{Limit, RETIRED_LIMITS, open_limits};

/// Checks a release's limits table without passing u64 values through JavaScript numbers.
pub fn check_recorded_limits(input: &str) -> pop_contracts::Result<()> {
    let recorded: Vec<Limit> = serde_json::from_str(input).map_err(|error| {
        pop_contracts::Error::new(pop_contracts::Code::InvalidBytes, format!("Missing or invalid limits: {error}"))
    })?;
    if recorded.is_empty() {
        return Err(pop_contracts::Error::new(pop_contracts::Code::InvalidBytes, "Empty limits table"));
    }
    let current = open_limits();
    let mut names = std::collections::HashSet::new();
    for old in recorded {
        if !names.insert(old.name.clone()) {
            return Err(pop_contracts::Error::new(
                pop_contracts::Code::InvalidBytes,
                format!("Duplicate limit {}", old.name),
            ));
        }
        match current.iter().find(|limit| limit.name == old.name) {
            Some(now) if !now.accepts_recorded(&old) => {
                return Err(pop_contracts::Error::new(
                    pop_contracts::Code::InvalidBytes,
                    format!(
                        "{} became stricter: recorded {:?} {}, current {:?} {}",
                        old.name, old.kind, old.value, now.kind, now.value
                    ),
                ));
            }
            None if !RETIRED_LIMITS.contains(&old.name.as_str()) => {
                return Err(pop_contracts::Error::new(
                    pop_contracts::Code::InvalidBytes,
                    format!("Recorded limit {} disappeared", old.name),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}
