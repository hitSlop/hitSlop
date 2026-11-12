//! Native sandbox and pipe adapter.
use crate::sandbox;
use crate::{Error, output_error};
use pop_contracts::evaluator::{INPUT, Job, OUTPUT};
use std::io::Read;
use std::time::{Duration, Instant};

pub fn child() {
    let mut output_limit = OUTPUT;
    let mut run = || -> Result<String, Error> {
        let mut input = Vec::new();
        std::io::stdin().take(INPUT as u64 + 1).read_to_end(&mut input).map_err(|e| Error::Output(e.to_string()))?;
        if input.len() > INPUT {
            return Err(Error::Output("Command input is too large".into()));
        }
        let job = Job::decode(&input).map_err(output_error)?;
        let limits = job.mode.limits();
        output_limit = limits.output;
        sandbox::restrict(limits.cpu_seconds)?;
        let deadline = Instant::now() + Duration::from_millis(limits.execution_ms);
        crate::evaluation::evaluate_job(job, move || Instant::now() > deadline)
    };
    let output = run().unwrap_or_else(|error| {
        serde_json::to_string(&crate::FailedReply::from(error)).expect("failure serialization")
    });
    if output.len() <= output_limit {
        println!("{output}");
    }
}
