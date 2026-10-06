use hitslop_core::{command, file};
use serde_json::{Value, json};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn refused(error: impl std::fmt::Display) -> Value {
    json!({"ok":false,"code":"rejected","reason":"invalid_request","error":error.to_string()})
}
fn route(request: Value, protocol: u64) -> Result<Value, Value> {
    let reply: Value =
        serde_json::from_str(&command::request(&request.to_string(), protocol, None)).map_err(refused)?;
    if reply["ok"] == true { Ok(reply) } else { Err(reply) }
}
fn state(path: &str, protocol: u64) -> Result<Value, Value> {
    route(json!({"method":"get","documentPath":path}), protocol).map(|r| r["state"].clone())
}
pub fn describe(path: &str, protocol: u64) -> Value {
    let run = || -> Result<Value, Value> {
        let state = state(path, protocol)?;
        let info = file::inspect(Path::new(path)).map_err(refused)?;
        let commands =
            file::commands(Path::new(path)).map_err(refused)?.map(|c| c.metadata).unwrap_or_else(|| json!({}));
        Ok(
            json!({"ok":true,"method":"describe","state":hitslop_core::describe::describe(info["manifest"].clone(),state,commands)}),
        )
    };
    run().unwrap_or_else(|e| e)
}
pub fn call(input: &str, protocol: u64) -> Value {
    run(input, protocol).unwrap_or_else(|e| e)
}
fn run(input: &str, protocol: u64) -> Result<Value, Value> {
    let request: Value = serde_json::from_str(input).map_err(refused)?;
    if !file::valid_call(&request) {
        return Err(refused("Invalid command request"));
    }
    let path = request["documentPath"].as_str().expect("validated");
    let name = request["command"].as_str().expect("validated");
    let mut state = state(path, protocol)?;
    let commands = file::commands(Path::new(path))
        .map_err(refused)?
        .ok_or_else(|| refused("This slop has no commands; use apply or batch"))?;
    if commands.metadata.get(name).is_none() {
        return Err(refused(format!("No command named {name}; run slop describe")));
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(refused)?.as_millis();
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(refused)?;
    let seed: [u32; 4] =
        std::array::from_fn(|i| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().expect("four bytes")));
    for attempt in 0..2 {
        let input = json!({"name":name,"args":request["args"],"value":state["value"],"descriptor":state["schema"],"now":now,"seed":seed});
        let output = crate::runner::run(commands.bundle.clone(), input.to_string()).map_err(refused)?;
        let reply: Value =
            serde_json::from_str(&output).map_err(|_| refused("Command runner stopped without a valid reply"))?;
        if reply["ok"] != true {
            return Err(refused(reply["error"].as_str().unwrap_or("Command failed")));
        }
        let applied = route(
            json!({"method":"batch","documentPath":path,"ops":reply["intents"].to_string(),"ifVersion":state["version"],"command":name}),
            protocol,
        );
        match applied {
            Ok(done) => return Ok(json!({"ok":true,"method":"call","result":reply["result"],"ids":done["ids"]})),
            // Only a definite pre-admission conflict is safe to replay. Time and random
            // seed belong to the invocation, so both evaluations receive identical ones.
            Err(error) if attempt == 0 && error["code"] == "rejected" && error["reason"] == "stale_base" => {
                state = self::state(path, protocol)?;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("second attempt returns")
}
