//! Native sandbox and pipe adapter.
use crate::{INPUT, Job, OUTPUT, sandbox};
use std::io::Read;
use std::time::{Duration, Instant};

pub fn child() {
    let mut output_limit = OUTPUT;
    let mut run = || -> Result<String, String> {
        let mut input = Vec::new();
        std::io::stdin().take(INPUT as u64 + 1).read_to_end(&mut input).map_err(|e| e.to_string())?;
        if input.len() > INPUT {
            return Err("Command input is too large".into());
        }
        let job = Job::decode(&input)?;
        let limits = job.mode.limits();
        output_limit = limits.output;
        sandbox::restrict(limits.cpu_seconds)?;
        let deadline = Instant::now() + Duration::from_millis(limits.execution_ms);
        crate::evaluation::evaluate_job(job, move || Instant::now() > deadline)
    };
    let output = run().unwrap_or_else(|error| {
        serde_json::to_string(&crate::protocol::JobFailure { ok: false, error }).expect("failure serialization")
    });
    if output.len() <= output_limit {
        println!("{output}");
    }
}
