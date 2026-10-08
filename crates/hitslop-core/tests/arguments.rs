use hitslop_core::arguments::Arguments;
use serde_json::{Value, json};

fn args(properties: Value) -> Arguments {
    Arguments::parse(&json!({"kind":"object","properties":properties}).to_string()).unwrap()
}

fn agrees(args: &Arguments, cases: impl IntoIterator<Item = (Value, bool)>) {
    let schema = args.json_schema();
    let oracle = jsonschema::validator_for(&schema).unwrap();
    for (value, accepted) in cases {
        assert_eq!(args.validate(&value).is_ok(), accepted, "descriptor: {value} against {schema}");
        assert_eq!(oracle.is_valid(&value), accepted, "projection: {value} against {schema}");
        // Documents and command arguments use the very same value checker.
        assert_eq!(
            hitslop_core::validate(&args.descriptor().to_string(), &value.to_string()).is_ok(),
            accepted,
            "document: {value}"
        );
    }
}

#[test]
fn projection_agrees_on_scalar_boundaries_and_json_number_spellings() {
    let cases = [
        (
            json!({"kind":"string","minLength":1,"maxLength":2}),
            vec![json!("a"), json!("😀"), json!("😀😀"), json!("e\u{301}")],
            vec![json!(""), json!("abc"), json!("😀😀😀"), json!(0), Value::Null],
        ),
        (json!({"kind":"string","minLength":0,"maxLength":0}), vec![json!("")], vec![json!("😀")]),
        (
            json!({"kind":"number","min":-1,"max":1}),
            vec![json!(-1), json!(0.5), json!(1), json!(-0.0)],
            vec![json!(-1.1), json!(1.1), json!(1e300), json!("1"), Value::Null],
        ),
        (json!({"kind":"number"}), vec![json!(1e300), json!(-1e300)], vec![json!(true)]),
        (
            json!({"kind":"integer","min":1,"max":3}),
            vec![json!(1), json!(3), json!(1.0), serde_json::from_str("3e0").unwrap()],
            vec![json!(0), json!(4), json!(1.5), json!(1e300), json!(true)],
        ),
        (
            json!({"kind":"integer"}),
            vec![json!(-0.0), json!(9007199254740991_i64), json!(-9007199254740991_i64)],
            vec![json!(9007199254740992_i64), json!(-9007199254740992_i64), json!(1e300)],
        ),
        (json!({"kind":"boolean"}), vec![json!(true), json!(false)], vec![json!(0), json!("true")]),
        (
            json!({"kind":"enum","values":["small","large"]}),
            vec![json!("small"), json!("large")],
            vec![json!("medium"), json!(1)],
        ),
        (
            json!({"kind":"list","item":{"kind":"integer","min":1}}),
            vec![json!([]), json!([1, 2, 3.0])],
            vec![json!([1, 0]), json!([1, "2"]), json!({})],
        ),
    ];
    for (node, good, bad) in cases {
        let contract = args(json!({"value":node}));
        agrees(
            &contract,
            good.into_iter()
                .map(|v| (json!({"value":v}), true))
                .chain(bad.into_iter().map(|v| (json!({"value":v}), false))),
        );
    }
}

#[test]
fn projection_agrees_on_closed_nested_objects_and_absent_optionals() {
    let contract = args(json!({
        "text":{"kind":"string"},
        "options":{"kind":"optional","inner":{"kind":"object","properties":{
            "labels":{"kind":"list","item":{"kind":"string"}},
            "enabled":{"kind":"optional","inner":{"kind":"boolean"}}
        }}}
    }));
    agrees(
        &contract,
        [
            (json!({"text":"a"}), true),
            (json!({"text":"a","options":{"labels":[]}}), true),
            (json!({"text":"a","options":{"labels":["x"],"enabled":false}}), true),
            (json!({}), false),
            (json!([]), false),
            (Value::Null, false),
            (json!({"text":"a","unknown":1}), false),
            (json!({"text":"a","options":null}), false),
            // An omitted list is empty.
            (json!({"text":"a","options":{}}), true),
            (json!({"text":"a","options":{"labels":[],"enabled":null}}), false),
            (json!({"text":"a","options":{"labels":[],"unknown":1}}), false),
        ],
    );
    agrees(&args(json!({})), [(json!({}), true), (json!({"extra":1}), false), (Value::Null, false)]);
}

#[test]
fn projection_includes_the_implicit_scalar_list_limit() {
    let contract = args(json!({"values":{"kind":"list","item":{"kind":"boolean"}}}));
    agrees(&contract, [(json!({"values":vec![true; 100_000]}), true), (json!({"values":vec![true; 100_001]}), false)]);
}

#[test]
fn every_forbidden_argument_kind_is_refused_recursively() {
    for node in [
        json!({"kind":"text"}),
        json!({"kind":"counter"}),
        json!({"kind":"record","value":{"kind":"string"}}),
        json!({"kind":"list","item":{"kind":"object","properties":{"name":{"kind":"string"}}}}),
        json!({"kind":"optional","inner":{"kind":"text"}}),
    ] {
        for nested in [false, true] {
            let mut node = node.clone();
            if nested {
                node = json!({"kind":"optional","inner":{"kind":"object","properties":{"child":node}}});
            }
            let schema = json!({"kind":"object","properties":{"value":node}});
            // Each is legal as document data; this refusal belongs to the argument subset.
            assert!(hitslop_core::AppSpec::data(&schema.to_string()).is_ok());
            let error = Arguments::parse(&schema.to_string()).unwrap_err();
            assert_eq!(error.code, hitslop_core::Code::InvalidSchema);
            assert_eq!(error.pointer(), if nested { "/value/child" } else { "/value" });
        }
    }
}

#[test]
fn descriptions_survive_projection_without_changing_validation() {
    let contract = Arguments::parse(
        &json!({
            "kind":"object","description":"Command input","properties":{
                "text":{"kind":"string","description":"Task text"},
                "enabled":{"kind":"optional","description":"Enable it","inner":{
                    "kind":"boolean","description":"Inner annotation"
                }}
            }
        })
        .to_string(),
    )
    .unwrap();
    let schema = contract.json_schema();
    assert_eq!(schema["description"], "Command input");
    assert_eq!(schema["properties"]["text"]["description"], "Task text");
    assert_eq!(schema["properties"]["enabled"]["description"], "Enable it");
    assert_eq!(contract.descriptor()["properties"]["enabled"]["inner"]["description"], "Inner annotation");
    agrees(&contract, [(json!({"text":"x"}), true), (json!({"text":false}), false)]);
}

#[test]
fn diagnostics_identify_missing_unknown_and_wrong_typed_values() {
    let contract = args(json!({"options":{"kind":"object","properties":{
        "labels":{"kind":"list","item":{"kind":"string"}},
        "name":{"kind":"string"}
    }}}));
    for (value, pointer) in [
        (json!({}), "/options"),
        (json!({"options":{}}), "/options/name"),
        (json!({"options":{"labels":["ok",3]}}), "/options/labels/1"),
        (json!({"options":{"labels":[],"x/y~z":1}}), "/options/x~1y~0z"),
    ] {
        assert_eq!(contract.validate(&value).unwrap_err().pointer(), pointer);
    }
    let error = args(json!({"text":{"kind":"string"}})).validate(&json!({"text":3})).unwrap_err();
    assert_eq!(format!("at {}: {}", error.pointer(), error.message), "at /text: must be a string");
}

#[test]
fn omitted_arguments_take_their_defaults_and_are_not_required() {
    let contract = args(json!({
        "text":{"kind":"string"},
        "done":{"kind":"boolean","default":false},
        "size":{"kind":"integer","min":1,"default":2},
        "labels":{"kind":"list","item":{"kind":"string"}}
    }));
    let schema = contract.json_schema();
    assert_eq!(schema["required"], json!(["text"]));
    assert_eq!(
        (&schema["properties"]["done"]["default"], &schema["properties"]["size"]["default"]),
        (&json!(false), &json!(2))
    );
    agrees(&contract, [(json!({"text":"a"}), true), (json!({"text":"a","size":0}), false), (json!({}), false)]);
    let mut value = json!({"text":"a"});
    contract.prepare(&mut value).unwrap();
    assert_eq!(value, json!({"text":"a","done":false,"size":2,"labels":[]}));
}

#[test]
fn row_arguments_are_row_ids_of_a_named_list() {
    let contract = Arguments::parse(
        &json!({"kind":"object","properties":{
            "task":{"kind":"row","list":"tasks","description":"The task to restore"}
        }})
        .to_string(),
    )
    .unwrap();
    assert_eq!(contract.row_lists().collect::<Vec<_>>(), ["tasks"]);
    let schema = contract.json_schema();
    assert_eq!(schema["properties"]["task"]["description"], "The task to restore");
    let oracle = jsonschema::validator_for(&schema).unwrap();
    for (value, accepted) in [
        (json!({"task":"a1_B-c"}), true),
        (json!({"task":"has space"}), false),
        (json!({"task":""}), false),
        (json!({"task":"x".repeat(65)}), false),
        (json!({"task":3}), false),
        (json!({}), false),
    ] {
        assert_eq!(contract.validate(&value).is_ok(), accepted, "{value}");
        assert_eq!(oracle.is_valid(&value), accepted, "projection: {value}");
    }
    assert_eq!(contract.validate(&json!({"task":"has space"})).unwrap_err().pointer(), "/task");
    for node in [
        json!({"kind":"row"}),
        json!({"kind":"row","list":"tasks","extra":1}),
        json!({"kind":"list","item":{"kind":"row","list":"tasks"}}),
    ] {
        let schema = json!({"kind":"object","properties":{"value":node}});
        let error = Arguments::parse(&schema.to_string()).unwrap_err();
        assert_eq!((error.code, error.pointer()), (hitslop_core::Code::InvalidSchema, "/value".into()));
    }
    // A document field is never a row reference.
    assert!(
        hitslop_core::AppSpec::data(
            &json!({"kind":"object","properties":{"task":{"kind":"row","list":"tasks"}}}).to_string()
        )
        .is_err()
    );
}

#[test]
fn row_arguments_are_required_top_level_fields_only() {
    for (node, pointer) in [
        (json!({"kind":"optional","inner":{"kind":"row","list":"tasks"}}), "/value"),
        (json!({"kind":"object","properties":{"task":{"kind":"row","list":"tasks"}}}), "/value/task"),
        (
            json!({"kind":"optional","inner":{"kind":"object","properties":{"task":{"kind":"row","list":"tasks"}}}}),
            "/value/task",
        ),
        (
            json!({"kind":"list","item":{"kind":"object","properties":{"task":{"kind":"row","list":"tasks"}}}}),
            "/value/item/task",
        ),
        (json!({"kind":"record","value":{"kind":"row","list":"tasks"}}), "/value/value"),
    ] {
        let schema = json!({"kind":"object","properties":{"value":node}});
        let error = Arguments::parse(&schema.to_string()).expect_err("rows resolve only at top level");
        assert_eq!((error.code, error.pointer()), (hitslop_core::Code::InvalidSchema, pointer.into()));
    }
}
