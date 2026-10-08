#![cfg(feature = "storage")]
use hitslop_core::app::{AppDefinition, WindowDefinition};
use hitslop_core::{Code, Document, build::BuildDeclaration};
use serde_json::{Value, json};

fn metadata() -> Value {
    json!({"slug":"fixture","title":"Fixture","description":"A document","author":{"name":"Author","url":"https://example.com"},"categories":["utilities"]})
}
fn definition() -> Value {
    json!({
        "window":{"kind":"standard","width":320,"height":240},
        "document":{"kind":"object","properties":{"title":{"kind":"string","minLength":1,"description":"Document title"}}},
        "theme":[{"token":"zinc","color":"#777777"},{"token":"accent","color":"#123456"}],
        "views":{"export":true,"icon":false},
        "commands":[{"name":"rename","description":"Rename the document","args":{
            "kind":"object","properties":{"title":{"kind":"string","minLength":1}}
        }}]
    })
}
fn read(metadata: &Value, definition: &Value) -> Result<AppDefinition, hitslop_core::Error> {
    AppDefinition::decode(1, 1, &metadata.to_string(), &definition.to_string())
}
/// Today's authoring rules for new metadata; a saved file's is only bounded (`read`).
fn author(metadata: &Value) -> Result<(), hitslop_core::Error> {
    hitslop_core::app::validate_metadata(&serde_json::from_value(metadata.clone()).unwrap())
}

#[test]
fn accepted_definition_keeps_order_and_reopens_with_the_same_document_contract() {
    let app = read(&metadata(), &definition()).unwrap();
    assert_eq!(app.theme().iter().map(|t| t.token.as_str()).collect::<Vec<_>>(), ["zinc", "accent"]);
    assert!(app.views().export);
    assert!(!app.views().icon);
    assert!(matches!(
        app.window(),
        WindowDefinition::Standard { width: 320, height: 240, resizable: true, lock_aspect: false, .. }
    ));
    app.commands()[0].args.validate(&json!({"title":"New title"})).unwrap();
    assert!(app.commands()[0].args.validate(&json!({"title":""})).is_err());
    let document = Document::create(app.spec(), r#"{"title":"First"}"#).unwrap();
    let reread = AppDefinition::decode(1, 1, &metadata().to_string(), &app.definition_json()).unwrap();
    let reopened = Document::open(reread.spec(), &document.checkpoint().unwrap(), &[]).unwrap();
    assert_eq!(document.state().unwrap(), reopened.state().unwrap());
    assert_eq!(app.definition_json(), reread.definition_json());
}

#[test]
fn bootstrap_receives_the_original_descriptor_including_descriptions() {
    let descriptor = r#"{ "kind":"object", "properties": {"z": {"kind":"string","description":"Keep this"}, "a": {"kind":"number","min":1.0}} }"#;
    let definition = format!(
        r#"{{"window":{{"kind":"standard","width":320,"height":240}},"document":{descriptor},"theme":[],"commands":[],"views":{{"export":false,"icon":false}}}}"#
    );
    let app = AppDefinition::decode(1, 1, &metadata().to_string(), &definition).unwrap();
    assert_eq!(app.document_json(), descriptor);
    let again = AppDefinition::decode(1, 1, &metadata().to_string(), &app.definition_json()).unwrap();
    assert_eq!(again.document_json(), descriptor);
}

#[test]
fn newer_markers_win_before_unknown_fields_or_unrepresentable_numbers() {
    for (format, abi) in [(2, 1), (1, 2), (u64::MAX, 1), (0, 2)] {
        let result = AppDefinition::decode(format, abi, r#"{"future":1e999}"#, r#"{"future":1e999}"#);
        assert_eq!(result.unwrap_err().code, Code::RequiresUpdate);
    }
    assert_eq!(AppDefinition::decode(0, 1, "{}", "{}").unwrap_err().code, Code::InvalidRequest);
    assert_eq!(AppDefinition::decode(1, 0, "{}", "{}").unwrap_err().code, Code::InvalidRequest);
    for (format, abi) in [(2, 1), (1, 2)] {
        let future = format!(r#"{{"packageFormat":{format},"runtimeABI":{abi},"unknown":1e999}}"#);
        assert_eq!(hitslop_core::build::BuildInput::decode(&future).unwrap_err().code, Code::RequiresUpdate);
    }
    assert_eq!(
        hitslop_core::build::BuildInput::decode(r#"{"packageFormat":1,"runtimeABI":1,"unknown":1e999}"#)
            .unwrap_err()
            .code,
        Code::InvalidRequest
    );
}

#[test]
fn metadata_errors_are_located_and_do_not_echo_authored_text() {
    for (path, bad) in [
        ("/slug", json!("Bad--slug")),
        ("/title", json!("secret".repeat(20))),
        ("/description", json!("")),
        ("/author/name", json!(" \n\t")),
        ("/categories", json!([])),
        ("/categories", json!(["utilities", "utilities"])),
    ] {
        let mut value = metadata();
        *value.pointer_mut(path).unwrap() = bad;
        let error = author(&value).unwrap_err();
        assert_eq!(error.pointer(), path);
        assert!(!error.message.contains("secret"));
    }
    for count in [80, 81] {
        let mut value = metadata();
        value["title"] = "😀".repeat(count).into();
        assert_eq!(author(&value).is_ok(), count == 80);
    }
    for (url, accepted) in [
        ("https://example.com/a%20b?q=1#part", true),
        ("http://localhost:3000", true),
        ("https://[::1]/", true),
        ("javascript:alert(1)", false),
        ("//example.com", false),
        ("https:///missing-host", false),
        ("https://", false),
        ("https://bad host", false),
        ("https://example.com/%zz", false),
        ("https://example.com\n", false),
    ] {
        let mut value = metadata();
        value["author"]["url"] = url.into();
        assert_eq!(author(&value).is_ok(), accepted, "{url:?}");
    }
}

#[test]
fn closed_types_refuse_unknown_fields_null_and_mixed_windows() {
    for (path, extra) in [
        ("", json!({"future":true})),
        ("/window", json!({"future":true})),
        ("/views", json!({"future":true})),
        ("/window", json!({"resizable":null})),
        ("/theme/0", json!({"future":true})),
        ("/commands/0", json!({"future":true})),
    ] {
        let mut value = definition();
        value.pointer_mut(path).unwrap().as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        assert!(read(&metadata(), &value).is_err(), "{path}: {extra}");
    }
    for path in ["/author", "/categories", "/author/url"] {
        let mut value = metadata();
        *value.pointer_mut(path).unwrap() = Value::Null;
        assert!(read(&value, &definition()).is_err(), "{path}");
    }
    let key = format!("media/{}.png", "a".repeat(64));
    let mut value = definition();
    value["window"] = json!({"kind":"skin","width":320,"height":240,"skin":key});
    assert!(matches!(read(&metadata(), &value).unwrap().window(), WindowDefinition::Skin { .. }));
    value["window"]["resizable"] = true.into();
    assert!(read(&metadata(), &value).is_err());
    value["window"].as_object_mut().unwrap().remove("resizable");
    value["window"]["skin"] = "media/../skin.png".into();
    assert_eq!(read(&metadata(), &value).unwrap_err().pointer(), "/window/skin");
}

#[test]
fn geometry_and_palette_use_the_existing_core_rules() {
    for (path, bad) in [
        ("/window/width", json!(239)),
        ("/window/height", json!(4097)),
        ("/window/width", json!(320.5)),
        ("/theme/0/color", json!("#FFFFFF")),
        ("/theme/0/token", json!("window-size")),
    ] {
        let mut value = definition();
        *value.pointer_mut(path).unwrap() = bad;
        assert!(read(&metadata(), &value).is_err(), "{path}");
    }
    let mut value = definition();
    value["window"]["shape"] = json!({"path":"M0 0 X"});
    assert_eq!(read(&metadata(), &value).unwrap_err().code, Code::InvalidShape);
    value["window"]["shape"] = json!({"path":"M0 0L100 100Z","viewBox":[100,100]});
    assert!(read(&metadata(), &value).is_ok());
    value["theme"][1]["token"] = "zinc".into();
    assert!(read(&metadata(), &value).is_err());
}

#[test]
fn commands_are_named_unique_and_use_the_descriptor_subset() {
    for name in ["", "Bad", "with-hyphen", "with_underscore"] {
        let mut value = definition();
        value["commands"][0]["name"] = name.into();
        assert_eq!(read(&metadata(), &value).unwrap_err().pointer(), "/commands/0/name");
    }
    let mut value = definition();
    let command = value["commands"][0].clone();
    value["commands"].as_array_mut().unwrap().push(command);
    assert_eq!(read(&metadata(), &value).unwrap_err().pointer(), "/commands/1/name");
    value = definition();
    value["commands"][0]["args"]["properties"]["title"] = json!({"kind":"text"});
    assert_eq!(read(&metadata(), &value).unwrap_err().pointer(), "/commands/0/args/title");
}

#[test]
fn row_arguments_name_a_list_of_objects_in_the_document() {
    let mut value = definition();
    value["document"]["properties"]["tasks"] =
        json!({"kind":"list","item":{"kind":"object","properties":{"text":{"kind":"text"}}}});
    value["document"]["properties"]["tags"] = json!({"kind":"list","item":{"kind":"string"}});
    for (list, accepted) in [("tasks", true), ("tags", false), ("title", false), ("missing", false)] {
        value["commands"][0]["args"]["properties"]["task"] = json!({"kind":"row","list":list});
        let result = read(&metadata(), &value);
        assert_eq!(result.is_ok(), accepted, "{list}");
        if let Err(error) = result {
            assert_eq!((error.code, error.pointer()), (Code::InvalidSchema, "/commands/0/args".into()));
        }
    }
}

#[test]
fn build_declarations_share_acceptance_and_check_initial_values() {
    let mut value = definition();
    value["metadata"] = metadata();
    value["initial"] = json!({"title":"Initial"});
    let mut input: BuildDeclaration = serde_json::from_value(value).unwrap();
    let app = AppDefinition::from_declaration(&input, None).unwrap();
    let reread =
        AppDefinition::decode(1, 1, &serde_json::to_string(app.metadata()).unwrap(), &app.definition_json()).unwrap();
    assert_eq!(reread.document_json(), app.document_json());
    input.initial = json!({"title":""});
    assert_eq!(AppDefinition::from_declaration(&input, None).unwrap_err().code, Code::OutOfRange);
    input.initial = json!({"title":"Valid"});
    let key = format!("media/{}.png", "a".repeat(64));
    input.window =
        serde_json::from_value(json!({"kind":"skin","width":320,"height":240,"image":format!("/assets/{key}")}))
            .unwrap();
    assert!(AppDefinition::from_declaration(&input, None).is_err());
    assert!(AppDefinition::from_declaration(&input, Some("media/wrong.png")).is_err());
    assert!(AppDefinition::from_declaration(&input, Some(&key)).is_ok());
}

#[test]
fn metadata_text_is_bounded_and_counts_unicode_code_points() {
    for count in [40, 41, 80, 81] {
        let mut value = metadata();
        value["title"] = "😀".repeat(count).into();
        assert_eq!(author(&value).is_ok(), count <= 80);
    }
    let mut invalid = metadata();
    invalid["title"] = "secret".repeat(50).into();
    let error = author(&invalid).unwrap_err();
    assert_eq!(error.pointer(), "/title");
    assert!(!error.message.contains("secret"));
    let mut padded = metadata().to_string();
    padded.push_str(&" ".repeat(65536 - padded.len()));
    assert!(AppDefinition::decode(1, 1, &padded, &definition().to_string()).is_ok());
    padded.push(' ');
    assert_eq!(AppDefinition::decode(1, 1, &padded, &definition().to_string()).unwrap_err().code, Code::TooLarge);
}
