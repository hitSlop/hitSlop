use pop_contracts::wire::engine::EngineRequest;

#[test]
fn future_app_payload_reaches_its_marker_check_unchanged() {
    let app =
        r#"{ "packageFormat":999,"runtimeABI":1,"manifest":{"future":1e999},"descriptor":{},"initial":{},"theme":{} }"#;
    let input = format!(r#"{{"method":"validateApp","stage":"/not-read","app":{app}}}"#);
    let EngineRequest::File(pop_contracts::wire::engine::FileRequest::ValidateApp { app: raw, .. }) =
        EngineRequest::parse(&input).unwrap()
    else {
        panic!()
    };
    assert_eq!(raw.get(), app);
    assert!(matches!(pop_storage::file::validate_app(raw.get(), std::path::Path::new("/not-read")),
        Err(pop_storage::store::Error::Rejected(error)) if error.code == pop_contracts::wire::Code::RequiresUpdate));
}

#[test]
fn future_app_markers_precede_current_required_and_unknown_fields() {
    for app in [
        r#"{"packageFormat":999,"runtimeABI":1}"#,
        r#"{"packageFormat":999,"runtimeABI":1,"futureField":1e999}"#,
        r#"{"packageFormat":1,"runtimeABI":999,"futureField":{"new":true}}"#,
    ] {
        let input = format!(r#"{{"method":"validateApp","stage":"/not-read","app":{app}}}"#);
        let EngineRequest::File(pop_contracts::wire::engine::FileRequest::ValidateApp { app, stage }) =
            EngineRequest::parse(&input).unwrap()
        else {
            panic!()
        };
        let error = pop_storage::file::validate_app(app.get(), std::path::Path::new(&stage)).unwrap_err();
        assert!(
            matches!(&error, pop_storage::store::Error::Rejected(inner)
            if inner.code == pop_contracts::wire::Code::RequiresUpdate),
            "{error}"
        );
    }
}
