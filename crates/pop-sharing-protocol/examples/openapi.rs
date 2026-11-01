fn main() {
    println!("{}", pop_sharing_protocol::http::openapi().to_pretty_json().expect("OpenAPI document"));
}
