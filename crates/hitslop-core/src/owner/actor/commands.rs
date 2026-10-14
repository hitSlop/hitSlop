//! Agent commands evaluate off the serial owner in a native child process.
use super::*;
use crate::engine::{False, True};
use serde::Deserialize;
use serde_json::Value;

pub(in crate::owner) struct Invocation {
    generation: u64,
    view: Option<String>,
    deadline: Option<Duration>,
    version: String,
    name: String,
    args: Value,
    now: u64,
    seed: [u32; 4],
    origin: Origin,
    attempt: u8,
    callback: Option<Completion>,
}
impl Invocation {
    pub(super) fn take_callback(&mut self) -> Completion {
        take(&mut self.callback)
    }
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
pub(in crate::owner) struct Evaluation {
    #[serde(rename = "ok")]
    _ok: True,
    intents: Vec<Value>,
    result: Value,
}
#[derive(Deserialize)]
struct Refusal {
    #[serde(rename = "ok")]
    _ok: False,
    error: String,
    /// The command called `refuse(message)`: a message for the person, not a fault.
    #[serde(default)]
    refused: bool,
}
/// The intents a command built for `abi` returns, decoded into the core's current
/// vocabulary. The reply stays raw JSON until this arm decodes it, so a later ABI whose
/// vocabulary differs translates its own reply here before decoding. ABI 1's are today's:
/// they change only additively (see the engineering contract), and the core never loosens
/// what it accepts to admit them.
fn current_intents(abi: u64, intents: Vec<Value>) -> Result<Vec<crate::Intent>> {
    match abi {
        1 => intents
            .into_iter()
            .map(|intent| serde_json::from_value(intent).map_err(|_| host_fault("it returned an invalid reply")))
            .collect(),
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

pub(super) fn worker(
    run: impl Fn(u64, &str, &str) -> std::result::Result<String, String>,
    work: mpsc::Receiver<Work>,
    sender: mpsc::Sender<Message>,
) {
    for work in work {
        let output = catch_unwind(AssertUnwindSafe(|| run(work.runtime_abi, &work.bundle, &work.input)))
            .unwrap_or_else(|_| Err("the evaluator panicked".into()));
        let done = work.finish(output);
        if let Err(mpsc::SendError(Message::Evaluated { mut invocation, .. })) = sender.send(done) {
            complete(take(&mut invocation.callback), Err(closed()));
        }
    }
}

impl Work {
    pub(super) fn finish(self, output: std::result::Result<String, String>) -> Message {
        let result = contained(|| {
            let output = output.map_err(host_fault)?;
            if output.len() > hitslop_runner::OUTPUT {
                return Err(host_fault("Command output is too large"));
            }
            serde_json::from_str(&output).map_err(|_| match serde_json::from_str::<Refusal>(&output) {
                Ok(Refusal { error, refused: true, .. }) => Failure::rejected(Code::Refused, error),
                Ok(refusal) => rejected(refusal.error),
                Err(_) => host_fault("it returned an invalid reply"),
            })
        });
        Message::Evaluated { invocation: self.invocation, result }
    }
}

impl Actor {
    pub(super) fn begin_command(
        &mut self,
        name: String,
        args_json: String,
        origin: Origin,
        view: Option<String>,
        deadline: Option<Duration>,
        callback: &mut Option<Completion>,
    ) -> Result<()> {
        self.mutation()?;
        if self.evaluating {
            return Err(rejected("Another command is running; wait for it to finish"));
        }
        let mut args: Value = crate::parse(&args_json)?;
        let command = self
            .store
            .app()
            .app
            .commands()
            .iter()
            .find(|command| command.name == name)
            .ok_or_else(|| rejected(format!("No command named {name}; run slop describe")))?;
        command.args.prepare(&mut args).map_err(|error| {
            rejected(format!("Invalid arguments for {name} at {}: {}", error.pointer(), error.message))
        })?;
        if self.evaluate.is_none() {
            return Err(Failure::rejected(Code::EngineError, "This hitSlop has no command evaluator"));
        }
        let now = self.unix_ms;
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
                complete(invocation.take_callback(), Err(error));
                return;
            }
        };
        let Some(evaluate) = &self.evaluate else {
            complete(
                invocation.take_callback(),
                Err(Failure::rejected(Code::EngineError, "This hitSlop has no command evaluator")),
            );
            return;
        };
        self.evaluating = true;
        if let Err(mpsc::SendError(Work { mut invocation, .. })) =
            evaluate.send(Work { runtime_abi: self.store.app().runtime_abi, invocation, bundle, input })
        {
            self.evaluating = false;
            complete(invocation.take_callback(), Err(closed()));
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
            if invocation.deadline.is_some_and(|d| d <= self.now) {
                return Err(rejected("Command expired before its intents were admitted"));
            }
            let output = result?;
            let intents = current_intents(self.store.app().runtime_abi, output.intents)?;
            let batch = crate::Batch { intents, if_version: Some(invocation.version.clone()) };
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
            && applied.as_ref().is_err_and(|e| e.kind == FailureKind::Rejected && e.reason == Some(Code::StaleBase))
        {
            invocation.attempt = 1;
            invocation.version = self.core.version();
            self.evaluate_command(invocation);
        } else {
            complete(invocation.take_callback(), applied);
        }
        self.release_held();
        self.sync_resume();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (tempfile::TempDir, Arc<store::Store>) {
        crate::registry::use_folder(&std::env::temp_dir().join("hitslop-test-registry")).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let assets = dir.path().join("assets");
        std::fs::create_dir(&assets).unwrap();
        std::fs::write(assets.join("ui.js"), "export default {}").unwrap();
        std::fs::write(assets.join("commands.js"), "globalThis.__slopCommands = {};").unwrap();
        let capture = include_str!("../../../../../tests/fixtures/checklist/document/assets/export.js");
        for role in ["preview", "icon"] {
            std::fs::write(assets.join(format!("{role}.js")), capture).unwrap();
        }
        let resources = ["ui", "commands", "preview", "icon"].map(|role| {
            let kind = if role == "commands" { "command" } else { "app" };
            json!({"kind":kind,"key":format!("{role}.js"),"mediaType":"text/javascript","path":format!("assets/{role}.js")})
        });
        let input = json!({
            "packageFormat":1,"runtimeABI":1,
            "declaration":{
                "metadata":{"slug":"fixture","title":"Fixture","description":"A document","author":{"name":"Fixture"},"categories":["utilities"]},
                "window":{"kind":"standard","width":320,"height":240},
                "document":{"kind":"object","properties":{}},"initial":{},"theme":[],
                "commands":[{"name":"run","description":"Run","args":{"kind":"object","properties":{}}}],
                "views":{"preview":true,"icon":true,"export":false}
            },
            "roles":{"ui":"ui.js","commands":"commands.js","preview":"preview.js","icon":"icon.js"},
            "resources":resources,
            "artwork":{}
        });
        let template = dir.path().join("Template.slop");
        crate::file::pack(&input.to_string(), dir.path(), &template, None).unwrap();
        let document = dir.path().join("Document.slop");
        crate::file::create_document(&template, &document).unwrap();
        let store = Arc::new(store::Store::open(&document, store::Mode::Document).unwrap());
        (dir, store)
    }

    #[test]
    fn an_evaluator_panic_completes_and_the_worker_runs_the_next_command() {
        let (_dir, store) = fixture();
        let (persist, _storage) = mpsc::channel();
        let mut actor = Actor::new(store, store::Mode::Document, Arc::new(|_| {}), persist).unwrap();
        let (evaluate, work) = mpsc::channel();
        let (sender, messages) = mpsc::channel();
        actor.evaluate = Some(evaluate);
        let thread = std::thread::spawn(move || {
            let calls = std::cell::Cell::new(0);
            worker(
                |_, _, _| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        panic!("injected evaluator panic");
                    }
                    Ok(json!({"ok":true,"intents":[],"result":"second command"}).to_string())
                },
                work,
                sender,
            );
        });
        for attempt in 0..2 {
            let (reply, result) = mpsc::channel();
            actor
                .begin_command(
                    "run".into(),
                    "{}".into(),
                    Origin::Agent,
                    None,
                    None,
                    &mut Some(Box::new(move |answer| reply.send(answer).unwrap())),
                )
                .unwrap();
            match messages.recv_timeout(Duration::from_secs(2)) {
                Ok(message) => {
                    actor.step(message);
                }
                Err(error) => {
                    assert!(!actor.evaluating, "evaluation stayed set after the worker stopped: {error}");
                    panic!("the evaluator did not complete: {error}");
                }
            }
            assert!(!actor.evaluating);
            let answer = result.recv_timeout(Duration::from_secs(2)).unwrap();
            if attempt == 0 {
                let failure = answer.unwrap_err();
                assert_eq!((failure.kind, failure.reason), (FailureKind::Rejected, Some(Code::EngineError)));
                assert_eq!(failure.message, "The command evaluator failed: the evaluator panicked");
            } else {
                assert!(
                    matches!(answer.unwrap(), Reply::Command { result_json, .. } if result_json == r#""second command""#)
                );
            }
        }
        let (reply, flushed) = mpsc::channel();
        actor.dispatch(Request::Flush, None, None, Box::new(move |answer| reply.send(answer).unwrap()));
        assert!(matches!(flushed.recv_timeout(Duration::from_secs(2)).unwrap().unwrap(), Reply::Unit));
        actor.evaluate = None;
        thread.join().unwrap();
    }
}
