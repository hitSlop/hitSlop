//! The evaluator runs in a new process with no inherited environment or owner handles.
//! The parent alone opens documents and routes mutations. A failed/oversized evaluation
//! cannot produce a partial edit, and the OS sandbox also contains interpreter bugs.
mod sandbox;
use rquickjs::{CatchResultExt, Context, Function, Runtime};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const INPUT: usize = 64 << 20;
const OUTPUT: usize = 4 << 20;
const TIME: Duration = Duration::from_secs(3);
const PRELUDE: &str = include_str!("prelude.generated.js");
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    bundle: String,
    request: String,
}

pub fn run(bundle: String, request: String) -> Result<String, String> {
    let input = serde_json::to_vec(&Input { bundle, request }).map_err(|e| e.to_string())?;
    if input.len() > INPUT {
        return Err("Command input is too large".into());
    }
    let mut child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg("--evaluate-command")
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
    let result = loop {
        match receiver.try_recv() {
            Ok((result, bytes)) => {
                break result.map_err(|e| e.to_string()).and_then(|_| {
                    if bytes.len() > OUTPUT {
                        Err("Command output is too large".into())
                    } else {
                        String::from_utf8(bytes).map_err(|e| e.to_string())
                    }
                });
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                break Err("Command runner stopped without a reply".into());
            }
            Err(_) => {}
        }
        if Instant::now() >= deadline {
            break Err("Command execution timed out".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    // Even an evaluator that wrote a valid reply cannot stay running.
    let _ = child.kill();
    let _ = child.wait();
    let _ = writer.join();
    let _ = reader.join();
    result
}

fn evaluate(input: Input) -> Result<String, String> {
    sandbox::restrict()?;
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    runtime.set_memory_limit(64 << 20);
    runtime.set_max_stack_size(512 << 10);
    let deadline = Instant::now() + Duration::from_secs(2);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let context = Context::full(&runtime).map_err(|e| e.to_string())?;
    let strict = |script: &str| format!("(function(){{\"use strict\";\n{script}\n}})();");
    context.with(|ctx| {
        ctx.eval::<(), _>(strict(PRELUDE)).catch(&ctx).map_err(|e| e.to_string())?;
        ctx.eval::<(), _>(strict(&input.bundle)).catch(&ctx).map_err(|e| e.to_string())?;
        let run: Function = ctx.globals().get("__hitslopRun").catch(&ctx).map_err(|e| e.to_string())?;
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
