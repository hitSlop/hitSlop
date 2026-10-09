//! Native sandbox and pipe adapter.
use crate::{INPUT, OUTPUT, sandbox};
use std::io::Read;
use std::time::{Duration, Instant};

pub fn child() {
    let run = || -> Result<String, String> {
        let mut input = Vec::new();
        std::io::stdin().take(INPUT as u64 + 1).read_to_end(&mut input).map_err(|e| e.to_string())?;
        if input.len() > INPUT {
            return Err("Command input is too large".into());
        }
        sandbox::restrict()?;
        let deadline = Instant::now() + Duration::from_millis(crate::EXECUTION_MS);
        crate::evaluate(&input, move || Instant::now() > deadline)
    };
    let output = run().unwrap_or_else(|error| serde_json::json!({"ok":false,"error":error}).to_string());
    if output.len() <= OUTPUT {
        println!("{output}");
    }
}
