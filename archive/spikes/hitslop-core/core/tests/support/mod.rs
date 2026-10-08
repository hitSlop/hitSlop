use serde_json::Value;
// Independent test consumer, not the publisher implementation.
pub fn apply_patches(value: &mut Value, ops: &Value) {
    for op in ops.as_array().unwrap() {
        let path = op["path"].as_array().unwrap();
        let mut target = &mut *value;
        let walk = if op["type"] == "remove" { &path[..path.len() - 1] } else { &path[..] };
        for segment in walk {
            if let Some(key) = segment.as_str() {
                target = &mut target[key];
            } else {
                target = target
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|v| v["$id"] == segment["id"])
                    .unwrap();
            }
        }
        match op["type"].as_str().unwrap() {
            "set" => *target = op["value"].clone(),
            "remove" => {
                target
                    .as_object_mut()
                    .unwrap()
                    .remove(path.last().unwrap().as_str().unwrap());
            }
            "insertRow" => target
                .as_array_mut()
                .unwrap()
                .insert(op["index"].as_u64().unwrap() as usize, op["value"].clone()),
            kind => {
                let rows = target.as_array_mut().unwrap();
                let i = rows.iter().position(|v| v["$id"] == op["id"]).unwrap();
                let row = rows.remove(i);
                if kind == "moveRow" {
                    rows.insert(op["index"].as_u64().unwrap() as usize, row);
                } else {
                    assert_eq!(kind, "deleteRow");
                }
            }
        }
    }
}
