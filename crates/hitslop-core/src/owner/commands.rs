//! One command path for page and CLI. Evaluation never blocks the serial owner.
use super::*;
use crate::engine::{False, True};
use serde::Deserialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct Invocation {
    generation: u64,
    view: Option<String>,
    deadline: Option<Instant>,
    version: String,
    name: String,
    args: Value,
    now: u64,
    seed: [u32; 4],
    origin: Origin,
    attempt: u8,
    callback: Option<Completion>,
}
const _: () = assert!(
    crate::RUNTIME_ABI == hitslop_runner::RUNTIME_ABI,
    "add the new evaluator ABI arm before raising the marker"
);

pub(super) struct Work {
    runtime_abi: u64,
    invocation: Invocation,
    bundle: Arc<String>,
    input: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Evaluation {
    #[serde(rename = "ok")]
    _ok: True,
    intents: Vec<crate::Intent>,
    result: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Refusal {
    #[serde(rename = "ok")]
    _ok: False,
    error: String,
}
/// The intents a command built for `abi` returns, in the core's current vocabulary. ABI 1's
/// are today's. A later ABI that changes the vocabulary translates an older one here; the
/// core never loosens what it accepts to admit them.
fn current_intents(abi: u64, intents: Vec<crate::Intent>) -> Result<Vec<crate::Intent>> {
    match abi {
        1 => Ok(intents),
        _ => Err(rejected("Unsupported command runtime ABI")),
    }
}
/// The evaluator, not the command, failed. A definite refusal: nothing was applied.
fn host_fault(error: impl std::fmt::Display) -> Failure {
    Failure::rejected(Code::EngineError, format!("The command evaluator failed: {error}"))
}
fn rejected(error: impl ToString) -> Failure {
    Failure::rejected(Code::InvalidRequest, error.to_string())
}

pub(super) fn worker(evaluator: Evaluator, work: mpsc::Receiver<Work>, sender: mpsc::Sender<Message>) {
    for Work { invocation, bundle, input, runtime_abi } in work {
        let result = contained(|| {
            // The evaluator process failed (it could not start, timed out, or replied
            // nothing usable): a host fault, not the command refusing. Nothing was applied.
            let output = evaluator.run(runtime_abi, &bundle, &input).map_err(host_fault)?;
            serde_json::from_str(&output).map_err(|_| match serde_json::from_str::<Refusal>(&output) {
                // The command threw: its own refusal.
                Ok(refusal) => rejected(refusal.error),
                Err(_) => host_fault("it returned an invalid reply"),
            })
        });
        if let Err(mpsc::SendError(Message::Evaluated { mut invocation, .. })) =
            sender.send(Message::Evaluated { invocation, result })
        {
            complete(take(&mut invocation.callback), Err(closed()));
        }
    }
}

impl Actor {
    pub(super) fn begin_command(
        &mut self,
        name: String,
        args_json: String,
        origin: Origin,
        view: Option<String>,
        deadline: Option<Instant>,
        callback: &mut Option<Completion>,
    ) -> Result<()> {
        self.mutation()?;
        if self.evaluating {
            return Err(rejected("Another command is running; wait for it to finish"));
        }
        let args: Value = crate::parse(&args_json)?;
        let command = self
            .store
            .app()
            .app
            .commands()
            .iter()
            .find(|command| command.name == name)
            .ok_or_else(|| rejected(format!("No command named {name}; run slop describe")))?;
        command.args.validate(&args).map_err(|error| {
            rejected(format!("Invalid arguments for {name} at {}: {}", error.pointer(), error.message))
        })?;
        if self.evaluate.is_none() {
            return Err(Failure::rejected(Code::EngineError, "This hitSlop has no command evaluator"));
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(rejected)?.as_millis() as u64;
        let mut bytes = [0; 16];
        getrandom::getrandom(&mut bytes).map_err(rejected)?;
        let seed = std::array::from_fn(|i| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().expect("four bytes")));
        self.evaluate_command(Invocation {
            generation: self.generation,
            version: self.core.version(),
            view,
            deadline,
            name,
            args,
            now,
            seed,
            origin,
            attempt: 0,
            callback: Some(take(callback)),
        });
        Ok(())
    }
    fn evaluate_command(&mut self, mut invocation: Invocation) {
        let prepare = || -> Result<(Arc<String>, String)> {
            let app = self.store.app();
            let bundle = app.commands.clone().ok_or_else(|| rejected("This app has no command program"))?;
            #[derive(serde::Serialize)]
            struct Input<'a> {
                name: &'a str,
                args: &'a Value,
                now: u64,
                seed: [u32; 4],
                value: Value,
                descriptor: &'a serde_json::value::RawValue,
            }
            let input = Input {
                name: &invocation.name,
                args: &invocation.args,
                now: invocation.now,
                seed: invocation.seed,
                value: self.core.projected(),
                descriptor: app.app.document_raw(),
            };
            Ok((bundle, crate::encode(&input)))
        };
        let (bundle, input) = match prepare() {
            Ok(input) => input,
            Err(error) => {
                complete(take(&mut invocation.callback), Err(error));
                return;
            }
        };
        let Some(evaluate) = &self.evaluate else {
            complete(
                take(&mut invocation.callback),
                Err(Failure::rejected(Code::EngineError, "This hitSlop has no command evaluator")),
            );
            return;
        };
        self.evaluating = true;
        if let Err(mpsc::SendError(Work { mut invocation, .. })) =
            evaluate.send(Work { runtime_abi: self.store.app().runtime_abi, invocation, bundle, input })
        {
            self.evaluating = false;
            complete(take(&mut invocation.callback), Err(closed()));
        }
    }
    pub(super) fn command_finished(&mut self, mut invocation: Invocation, result: Result<Evaluation>) {
        self.evaluating = false;
        // A runtime failure is a definite refusal: no intents have been admitted.
        // Only a stale version retries, once, with the same clock and random seed.
        let applied = catch_unwind(AssertUnwindSafe(|| -> Result<Reply> {
            self.admit(false, invocation.view.as_deref())?;
            self.mutation()?;
            if invocation.generation != self.generation {
                return Err(replaced());
            }
            if invocation.deadline.is_some_and(|d| d <= Instant::now()) {
                return Err(rejected("Command expired before its intents were admitted"));
            }
            let output = result?;
            let intents = current_intents(self.store.app().runtime_abi, output.intents)?;
            let batch = crate::Batch { intents, ifVersion: Some(invocation.version.clone()), base: None };
            let accepted = self.edited(|core| core.apply_command(batch, invocation.origin, &invocation.name))?;
            self.accepted(accepted.sequence, accepted.publication, accepted.theme_changed);
            Ok(Reply::Command {
                sequence: accepted.sequence,
                ids: accepted.ids,
                result_json: output.result.to_string(),
            })
        }))
        .unwrap_or_else(|_| {
            self.invalidated = true;
            self.fail(poisoned(), u64::MAX);
            Err(poisoned())
        });
        if invocation.attempt == 0
            && applied
                .as_ref()
                .is_err_and(|e| e.kind == FailureKind::Rejected && e.reason.as_deref() == Some("stale_base"))
        {
            invocation.attempt = 1;
            invocation.version = self.core.version();
            self.evaluate_command(invocation);
        } else {
            complete(take(&mut invocation.callback), applied);
        }
    }
}
