//! Authored code has no host APIs and cannot exhaust the parent. No file is involved:
//! this tests the restricted child's boundary independently of mutation routing.
use hitslop_runner::Evaluator;
use serde_json::{Value, json};
fn evaluate(code: &str) -> Value {
    run(code).unwrap()
}
fn run(code: &str) -> Result<Value, String> {
    let args = json!({});
    let bundle = format!(
        "globalThis.__slopCommands = {{[Symbol.for('slop.command-set')]:{{specs:{{probe:{{args:{args},run(ctx){{{code}}}}}}}}}}};"
    );
    let request = json!({"name":"probe","args":{},"value":{"count":0},"descriptor":{"kind":"object","properties":{"count":{"kind":"counter"}}},"now":1234,"seed":[1,2,3,4]});
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_hitslop-evaluator").into(), vec![]).unwrap();
    let output = evaluator.run(1, &bundle, &request.to_string())?;
    serde_json::from_str(&output).map_err(|e| format!("{e}: {output}"))
}
#[test]
fn restricted_commands_collect_intents_with_only_host_time_and_seed() {
    let code = "ctx.tx.fields.count.increment(2); return {now:ctx.now,random:ctx.random(),apis:[typeof process,typeof Bun,typeof fetch,typeof require,typeof WebSocket]};";
    let result = evaluate(code);
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["intents"], json!([{"type":"increment","path":["count"],"by":2}]));
    assert_eq!(result["result"]["now"], 1234);
    assert_eq!(result["result"]["apis"], json!(vec!["undefined"; 5]));
    assert_eq!(evaluate(code), result, "same invocation is deterministic");
}
#[test]
fn runner_refuses_async_mutation_and_ambient_clock() {
    for code in [
        "ctx.current.count = 7",
        "return Date.now()",
        "return new (Date.prototype.constructor)()",
        "return new (new Date(0).constructor)().getTime()",
        "return Date()",
        "return new Date()",
        "return Math.random()",
        "return Promise.resolve(2)",
        "ctx.tx.fields.count.increment(1); throw Error('refused')",
    ] {
        let result = evaluate(code);
        assert_eq!(result["ok"], false, "{code}: {result}");
        assert!(result.get("intents").is_none(), "failure exposes no staged writes");
    }
}
#[test]
fn explicit_date_constructors_keep_their_arguments() {
    let result = evaluate(
        "return {year:new Date(2020, 5, 12).getFullYear(),day:new Date(2020, 5, 12).getDate(),epoch:new Date(1234).getTime()}",
    );
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["result"], json!({"year":2020,"day":12,"epoch":1234}));
}

#[test]
fn module_globals_do_not_survive_an_invocation() {
    let code = "globalThis.counter=(globalThis.counter??0)+1; return globalThis.counter;";
    assert_eq!(evaluate(code)["result"], 1);
    assert_eq!(evaluate(code)["result"], 1);
}
#[test]
fn runner_bounds_cpu_and_memory() {
    for code in [
        "while(true){}",
        "const a=[]; while(true) a.push(new Array(100000).fill('x'));",
        "return (function recurse(){return recurse()})()",
    ] {
        let start = std::time::Instant::now();
        match run(code) {
            Ok(result) => assert_eq!(result["ok"], false, "{result}"),
            // The parent watchdog is also a valid limit outcome when process startup
            // leaves too little time for the child's interruption reply to arrive.
            Err(error) => assert_eq!(error, "Command execution timed out", "{code}"),
        }
        assert!(start.elapsed() < std::time::Duration::from_millis(hitslop_runner::WATCHDOG_MS * 2));
        assert_eq!(evaluate("return 42")["result"], 42, "the host remains usable after a bounded failure");
    }
}
