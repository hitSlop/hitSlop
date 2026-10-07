//! The evaluator runs in a new process with no inherited environment or owner handles.
//! The parent alone opens documents and routes mutations. A failed/oversized evaluation
//! cannot produce a partial edit, and the OS sandbox also contains interpreter bugs.
mod sandbox;
use rquickjs::{CatchResultExt, Context, Function, Runtime};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const INPUT: usize = 64 << 20;
const OUTPUT: usize = 4 << 20;
const TIME: Duration = Duration::from_secs(3);
pub const RUNTIME_ABI: u64 = 1;
fn prelude(abi: u64) -> Result<&'static str, String> {
    match abi {
        1 => Ok(include_str!("abi/1.generated.js")),
        _ => Err("Unsupported command runtime ABI".into()),
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    #[serde(rename = "runtimeABI")]
    runtime_abi: u64,
    bundle: String,
    request: String,
    #[serde(default)]
    mode: Mode,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Mode {
    #[default]
    Command,
    Definition,
}

/// A host-configured child, never an in-process evaluator. Every call starts a
/// fresh process and QuickJS realm; the host retains all document authority.
#[derive(Clone, Debug)]
pub struct Evaluator {
    executable: PathBuf,
    arguments: Vec<String>,
}
impl Evaluator {
    pub fn new(executable: PathBuf, arguments: Vec<String>) -> Result<Self, String> {
        if !executable.is_absolute() {
            return Err("Evaluator path must be absolute".into());
        }
        Ok(Self { executable, arguments })
    }
    pub fn run(&self, runtime_abi: u64, bundle: String, request: String) -> Result<String, String> {
        let input = serde_json::to_vec(&Input { runtime_abi, bundle, request, mode: Mode::Command })
            .map_err(|e| e.to_string())?;
        if input.len() > INPUT {
            return Err("Command input is too large".into());
        }
        let mut child = Command::new(&self.executable)
            .args(&self.arguments)
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut stdin = child.stdin.take().expect("piped input");
        let stdout = child.stdout.take().expect("piped output");
        let writer = std::thread::spawn(move || stdin.write_all(&input));
        let (sender, receiver) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut output = Vec::new();
            let result = stdout.take(OUTPUT as u64 + 1).read_to_end(&mut output);
            let _ = sender.send((result, output));
        });
        let deadline = Instant::now() + TIME;
        let result = match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok((result, bytes)) => result.map_err(|e| e.to_string()).and_then(|_| {
                if bytes.len() > OUTPUT {
                    Err("Command output is too large".into())
                } else {
                    String::from_utf8(bytes).map_err(|e| e.to_string())
                }
            }),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err("Command runner stopped without a reply".into())
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err("Command execution timed out".into()),
        };
        // Even an evaluator that wrote a valid reply cannot stay running.
        let _ = child.kill();
        let _ = child.wait();
        let _ = writer.join();
        let _ = reader.join();
        result
    }
}

fn evaluate(input: Input) -> Result<String, String> {
    let prelude = prelude(input.runtime_abi)?;
    sandbox::restrict()?;
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    runtime.set_memory_limit(64 << 20);
    runtime.set_max_stack_size(512 << 10);
    let deadline = Instant::now() + Duration::from_secs(2);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let context = Context::full(&runtime).map_err(|e| e.to_string())?;
    let strict = |script: &str| format!("(function(){{\"use strict\";\n{script}\n}})();");
    context.with(|ctx| {
        ctx.eval::<(), _>(strict(prelude)).catch(&ctx).map_err(|e| e.to_string())?;
        ctx.eval::<(), _>(strict(&input.bundle)).catch(&ctx).map_err(|e| e.to_string())?;
        let function = match input.mode {
            Mode::Command => "__hitslopRun",
            Mode::Definition => "__hitslopDescribe",
        };
        let run: Function = ctx.globals().get(function).catch(&ctx).map_err(|e| e.to_string())?;
        run.call::<_, String>((input.request,)).catch(&ctx).map_err(|e| e.to_string())
    })
}

pub fn child() {
    let run = || -> Result<String, String> {
        let mut input = Vec::new();
        std::io::stdin().take(INPUT as u64 + 1).read_to_end(&mut input).map_err(|e| e.to_string())?;
        if input.len() > INPUT {
            return Err("Command input is too large".into());
        }
        evaluate(serde_json::from_slice(&input).map_err(|e| e.to_string())?)
    };
    let output = run().unwrap_or_else(|error| serde_json::json!({"ok":false,"error":error}).to_string());
    if output.len() <= OUTPUT {
        println!("{output}");
    }
}
