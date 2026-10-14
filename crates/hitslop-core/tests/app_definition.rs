#![cfg(feature = "storage")]
use hitslop_core::app::{AppDefinition, Capability, StandardFrame, WindowFrame};
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
        "views":{"preview":true,"export":true,"icon":true},
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
    assert!(app.views().preview);
    assert!(app.views().icon);
    assert_eq!((app.window().width, app.window().height), (320, 240));
    assert!(matches!(
        app.window().frame,
        WindowFrame::Standard { frame: StandardFrame { resizable: true, lock_aspect: false, .. } }
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
fn reserved_theme_names_are_refused_at_authoring_and_open() {
    for token in ["window-radius", "host-accent", "platform-accent", "ordinary-accent"] {
        let mut value = definition();
        value["theme"] = json!([{"token":token,"color":"#123456"}]);
        let opened = read(&metadata(), &value);
        value["metadata"] = metadata();
        value["initial"] = json!({"title":"Initial"});
        let declaration: BuildDeclaration = serde_json::from_value(value).unwrap();
        let authored = AppDefinition::from_declaration(&declaration, None);
        if token == "ordinary-accent" {
            assert_eq!(opened.unwrap().theme()[0].token, token);
            assert_eq!(authored.unwrap().theme()[0].token, token);
        } else {
            assert_eq!(opened.unwrap_err().code, Code::InvalidKey, "{token}");
            assert_eq!(authored.unwrap_err().code, Code::InvalidKey, "{token}");
        }
    }
}

#[test]
fn widget_role_and_refresh_policy_survive_pack_and_reopen() {
    assert!(read(&metadata(), &definition()).unwrap().views().widget.is_none());
    for options in [json!({}), json!({"refreshMinutes":15}), json!({"refreshMinutes":1440})] {
        let mut value = definition();
        value["views"]["widget"] = options.clone();
        value["metadata"] = metadata();
        value["initial"] = json!({"title":"Initial"});
        let declaration: BuildDeclaration = serde_json::from_value(value).unwrap();
        let app = AppDefinition::from_declaration(&declaration, None).unwrap();
        let reopened = AppDefinition::decode(1, 1, &metadata().to_string(), &app.definition_json()).unwrap();
        assert_eq!(serde_json::to_value(reopened.views().widget).unwrap(), options);
    }
    for bad in [json!(true), json!(null), json!({"refreshMinutes":15.5}), json!({"refreshMinutes":null})] {
        let mut value = definition();
        value["views"]["widget"] = bad.clone();
        assert!(read(&metadata(), &value).is_err(), "{bad}");
    }
    for minutes in [0, 14, 1441, u32::MAX] {
        let mut value = definition();
        value["views"]["widget"] = json!({"refreshMinutes": minutes});
        let opened = read(&metadata(), &value).expect("host scheduling policy must not reject stored documents");
        assert_eq!(opened.views().widget.unwrap().refresh_minutes, Some(minutes));
        value["metadata"] = metadata();
        value["initial"] = json!({"title":"Initial"});
        let declaration: BuildDeclaration = serde_json::from_value(value).unwrap();
        assert!(AppDefinition::from_declaration(&declaration, None).is_err());
    }
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
fn null_and_mistyped_members_are_refused() {
    for (path, extra) in [
        ("/window", json!({"resizable":null})),
        ("/window", json!({"fullscreenable":null})),
        ("/window", json!({"fullscreenable":"true"})),
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
    assert!(matches!(read(&metadata(), &value).unwrap().window().frame, WindowFrame::Skin { .. }));
    value["window"]["skin"] = "media/../skin.png".into();
    assert_eq!(read(&metadata(), &value).unwrap_err().pointer(), "/window/skin");
    // A kind of window this build does not know cannot be read at all.
    value["window"]["kind"] = "future".into();
    assert!(read(&metadata(), &value).is_err());
}

/// A definition that reaches every container the stored reader owns.
fn rich() -> Value {
    let mut value = definition();
    value["window"] = json!({"kind":"standard","width":320,"height":240,"fullscreenable":true,"resizable":false,
        "lockAspect":true,"background":"glass",
        "shape":{"path":"M0 0L100 100Z","viewBox":[100,100],"fillRule":"evenodd"}});
    value["views"]["widget"] = json!({"refreshMinutes":30});
    value["capabilities"] = json!(["camera"]);
    value
}
/// `value` with a member no build defines in the container at JSON pointer `path`.
fn with_member(mut value: Value, path: &str) -> Value {
    let member = json!({"nested":[1,{"deeper":null}],"flag":true});
    value.pointer_mut(path).unwrap().as_object_mut().unwrap().insert("future".into(), member);
    value
}
/// Everything an opened app exposes, so two reads of it can be compared.
fn reading(app: &AppDefinition) -> Value {
    json!({
        "stored": app.definition_json(),
        "document": app.document_json(),
        "window": app.window(),
        "page": app.page_window(),
        "views": app.views(),
        "theme": app.theme(),
        "commands": app.command_metadata(),
        "capabilities": app.capabilities(),
    })
}

// Failure: every container of the stored definition refused a member it did not know, so a
// later build could not add an optional one without a new package format. Oracle: the read
// of the row with the member equals the read without it.
#[test]
fn unknown_members_of_the_stored_definition_are_ignored() {
    let known = reading(&read(&metadata(), &rich()).unwrap());
    let mut refused = vec![];
    for path in ["", "/window", "/window/shape", "/views", "/views/widget", "/theme/0", "/commands/0"] {
        match read(&metadata(), &with_member(rich(), path)) {
            Ok(app) => assert_eq!(reading(&app), known, "{path}"),
            Err(error) => refused.push(format!("{path}: {error}")),
        }
    }
    assert!(refused.is_empty(), "{refused:#?}");
    // A skin window ignores what only a standard window defines, as it does any other member.
    let mut skin = definition();
    skin["window"] = json!({"kind":"skin","width":320,"height":240,"fullscreenable":true,
        "skin":format!("media/{}.png", "a".repeat(64))});
    let known = reading(&read(&metadata(), &skin).unwrap());
    for (name, member) in [("future", json!({"deeper":[1]})), ("resizable", json!(true))] {
        let mut value = skin.clone();
        value["window"][name] = member;
        assert_eq!(reading(&read(&metadata(), &value).unwrap()), known, "{name}");
    }
    // What is known stays checked; only an unknown member is passed over.
    let mut widget = with_member(rich(), "/views/widget");
    widget["views"]["widget"]["refreshMinutes"] = "soon".into();
    assert!(read(&metadata(), &widget).is_err());
}

// The grammars that decide what a value may be stay exact: an option this build does not
// know would weaken validation if it were ignored.
#[test]
fn an_unknown_option_inside_a_descriptor_is_still_refused() {
    for path in ["/document", "/document/properties/title", "/commands/0/args", "/commands/0/args/properties/title"] {
        let value = with_member(definition(), path);
        assert!(read(&metadata(), &value).is_err(), "{path}");
    }
}

// Authoring input is not a saved file: a member the engine does not know there is an SDK
// newer than the engine, which must be told so rather than have its output dropped.
#[test]
fn authoring_input_stays_exact_about_unknown_members() {
    let mut input = rich();
    input["metadata"] = metadata();
    input["initial"] = json!({"title":"Initial"});
    serde_json::from_value::<BuildDeclaration>(input.clone()).unwrap();
    for path in ["", "/window", "/window/shape", "/views", "/views/widget", "/theme/0", "/commands/0"] {
        let declaration = serde_json::from_value::<BuildDeclaration>(with_member(input.clone(), path));
        assert!(declaration.is_err(), "{path}");
    }
}

// The stored definition is persisted bytes. Reading it into the format's own types and back
// reproduces them exactly, so what a writer stored does not depend on the host model.
#[test]
fn an_accepted_definition_re_encodes_to_the_bytes_it_was_stored_as() {
    let skin = format!("media/{}.png", "a".repeat(64));
    let standard = r##"{"kind":"standard","width":320.0,"height":240.0,"fullscreenable":true,"resizable":false,"lockAspect":true,"background":"glass","shape":{"path":"M0 0L100 100Z","viewBox":[100.0,100.0],"fillRule":"evenodd"}}"##;
    let skinned = format!(r#"{{"kind":"skin","width":320.0,"height":240.0,"fullscreenable":true,"skin":"{skin}"}}"#);
    for (window, widget) in [(standard, r#","widget":{"refreshMinutes":30}"#), (skinned.as_str(), r#","widget":{}"#)] {
        let stored = [
            r#"{"window":"#,
            window,
            r##","document":{"kind":"object","properties":{"title":{"kind":"string","minLength":1,"description":"Document title"}}},"theme":[{"token":"zinc","color":"#777777"},{"token":"accent","color":"#123456"}],"commands":[{"name":"rename","description":"Rename the document","args":{"kind":"object","properties":{"title":{"kind":"string","minLength":1}}}}],"views":{"preview":true,"export":true,"icon":true"##,
            widget,
            r#"},"capabilities":["camera"]}"#,
        ]
        .concat();
        let app = AppDefinition::decode(1, 1, &metadata().to_string(), &stored).unwrap();
        assert_eq!(app.definition_json(), stored);
    }
}

#[test]
fn fullscreen_is_opt_in_for_both_window_kinds_and_survives_the_page_and_file_projections() {
    for skin in [false, true] {
        for enabled in [None, Some(false), Some(true)] {
            let mut value = definition();
            if skin {
                value["window"] = json!({"kind":"skin","width":320,"height":240,
                    "skin":format!("media/{}.png", "a".repeat(64))});
            }
            if let Some(enabled) = enabled {
                value["window"]["fullscreenable"] = enabled.into();
            }
            let app = read(&metadata(), &value).unwrap();
            let actual = app.window().fullscreenable;
            assert_eq!(actual, enabled.unwrap_or(false));
            let page = serde_json::to_value(app.page_window()).unwrap();
            assert_eq!(page.get("fullscreenable"), value["window"].get("fullscreenable"));
            let saved = app.definition_json();
            let reopened = AppDefinition::decode(1, 1, &metadata().to_string(), &saved).unwrap();
            assert_eq!(reopened.definition_json(), saved);
            assert_eq!(serde_json::to_value(reopened.page_window()).unwrap(), page);
            let resolved = serde_json::to_value(app.window()).unwrap();
            assert_eq!(resolved["fullscreenable"], enabled.unwrap_or(false));
            assert_eq!(serde_json::to_value(reopened.window()).unwrap(), resolved);
            // Authoring input has the same optional-boolean rule, including skins.
            let mut invalid = page.clone();
            invalid["fullscreenable"] = Value::Null;
            assert!(serde_json::from_value::<hitslop_core::build::WindowInput>(invalid).is_err());
            assert!(serde_json::from_value::<hitslop_core::build::WindowInput>(page).is_ok());
        }
    }
}

#[test]
fn fullscreen_reflows_only_resizable_unshaped_unlocked_windows() {
    for (extra, fits) in [
        (json!({}), false),
        (json!({"resizable":false}), true),
        (json!({"lockAspect":true}), true),
        (json!({"shape":"50%"}), true),
    ] {
        let mut value = definition();
        value["window"].as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let app = read(&metadata(), &value).unwrap();
        let WindowFrame::Standard { frame } = &app.window().frame else { unreachable!() };
        assert_eq!(frame.fullscreen_fit, fits);
    }
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
    let app = read(&metadata(), &value).unwrap();
    let resolved = serde_json::to_value(app.window()).unwrap();
    assert_eq!(resolved["frame"]["frame"]["radius"], "0");
    assert_eq!(
        resolved["frame"]["frame"]["shape"],
        json!({
            "kind":"path", "viewBoxWidth":100.0, "viewBoxHeight":100.0, "evenOdd":false,
            "segments":[{"kind":"move","x":0.0,"y":0.0},{"kind":"line","x":100.0,"y":100.0},{"kind":"close"}]
        })
    );
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
fn row_arguments_are_not_accepted() {
    let mut value = definition();
    value["commands"][0]["args"]["properties"]["task"] = json!({"kind":"row","list":"tasks"});
    assert_eq!(read(&metadata(), &value).unwrap_err().code, Code::InvalidRequest);
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
fn every_capability_is_in_all_and_is_granted_when_declared() {
    let expected = [Capability::Camera, Capability::Microphone, Capability::Speech];
    assert_eq!(Capability::ALL, expected);
    let mut value = definition();
    value["capabilities"] = serde_json::to_value(expected).unwrap();
    assert_eq!(read(&metadata(), &value).unwrap().capabilities(), expected);
}

#[test]
fn capabilities_are_declared_canonically_refused_unknown_at_authoring_and_ignored_unknown_on_open() {
    let declaration = |capabilities: Value| {
        let mut value = definition();
        value["metadata"] = metadata();
        value["initial"] = json!({"title":"Initial"});
        if !capabilities.is_null() {
            value["capabilities"] = capabilities;
        }
        value
    };
    let declare = |capabilities: Value| serde_json::from_value::<BuildDeclaration>(declaration(capabilities));
    // Absent is none, and stores nothing: the definition reads as it did before the field.
    let none = AppDefinition::from_declaration(&declare(Value::Null).unwrap(), None).unwrap();
    assert!(none.capabilities().is_empty());
    assert!(!none.definition_json().contains("capabilities"));
    // Packing stores one sorted spelling per capability; the stored form reopens unchanged.
    let app = AppDefinition::from_declaration(&declare(json!(["microphone", "camera", "microphone"])).unwrap(), None)
        .unwrap();
    assert_eq!(app.capabilities(), [Capability::Camera, Capability::Microphone]);
    let stored: Value = serde_json::from_str(&app.definition_json()).unwrap();
    assert_eq!(stored["capabilities"], json!(["camera", "microphone"]));
    let reread = AppDefinition::decode(1, 1, &metadata().to_string(), &app.definition_json()).unwrap();
    assert_eq!(reread.capabilities(), app.capabilities());
    assert_eq!(reread.definition_json(), app.definition_json());
    // Authoring refuses a capability this build does not implement, and any non-string.
    let input = |capabilities: Value| {
        json!({"packageFormat":1,"runtimeABI":1,"declaration":declaration(capabilities),
            "roles":{"ui":"ui.js"},"resources":[],"artwork":{}})
        .to_string()
    };
    assert!(hitslop_core::build::BuildInput::decode(&input(json!(["speech"]))).is_ok());
    for bad in [json!(["camera", "telepathy"]), json!(["Camera"]), json!([null]), json!("camera")] {
        assert!(declare(bad.clone()).is_err(), "{bad}");
        let error = hitslop_core::build::BuildInput::decode(&input(bad.clone())).unwrap_err();
        assert_eq!(error.code, Code::InvalidRequest, "{bad}");
    }
    let error = hitslop_core::build::BuildInput::decode(&input(json!(["telepathy"]))).unwrap_err();
    assert!(error.message.contains("telepathy") && error.message.contains("microphone"), "{}", error.message);
    // Opening never grants what it does not know: a later build's capability is left out,
    // the rest are sorted and unique, and the saved spelling is kept as it was.
    let mut value = definition();
    value["capabilities"] = json!(["speech", "telepathy", "camera", "camera"]);
    let opened = read(&metadata(), &value).unwrap();
    assert_eq!(opened.capabilities(), [Capability::Camera, Capability::Speech]);
    let saved: Value = serde_json::from_str(&opened.definition_json()).unwrap();
    assert_eq!(saved["capabilities"], value["capabilities"]);
    // The stored shape is still a bounded list of strings.
    for bad in [json!([1]), json!([null]), json!("camera"), json!({"camera":true})] {
        value["capabilities"] = bad.clone();
        assert!(read(&metadata(), &value).is_err(), "{bad}");
    }
    value["capabilities"] = json!(["x".repeat(70_000)]);
    assert_eq!(read(&metadata(), &value).unwrap_err().code, Code::TooLarge);
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
