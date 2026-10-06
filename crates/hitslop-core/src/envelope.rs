//! Native acceptance of platform envelopes over the TypeBox-generated contracts: socket
//! requests, replies and discovery files, and page bridge requests. The schemas are the
//! only rules; this module parses bounded JSON and answers whether an envelope matches.

#[jsonschema::validator(
    path = "../../packages/hitslop/generated/socket-request.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = true, validate = false, iter_errors = false }
)]
struct SocketRequest;

#[jsonschema::validator(
    path = "../../packages/hitslop/generated/socket-reply.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = true, validate = false, iter_errors = false }
)]
struct SocketReply;

#[jsonschema::validator(
    path = "../../packages/hitslop/generated/socket-discovery.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = true, validate = false, iter_errors = false }
)]
struct SocketDiscovery;

#[jsonschema::validator(path = "../../packages/hitslop/generated/socket-hello.schema.json", draft = Draft7)]
struct SocketHello;

#[jsonschema::validator(path = "../../packages/hitslop/generated/socket-hello-success.schema.json", draft = Draft7)]
struct SocketHelloSuccess;

#[jsonschema::validator(
    path = "../../packages/hitslop/generated/page-request.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = true, validate = false, iter_errors = false }
)]
struct PageRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Envelope {
    SocketRequest,
    SocketReply,
    SocketDiscovery,
    PageRequest,
}

/// Whether `json` is a well-formed envelope of `kind`. The largest is an attachment upload
/// on the socket; anything bigger is refused before parsing.
pub fn is_valid(kind: Envelope, json: &[u8]) -> bool {
    if json.len() > crate::wire::SOCKET_ATTACHMENT {
        return false;
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(json) else {
        return false;
    };
    match kind {
        Envelope::SocketRequest => SocketRequest::is_valid(&value),
        Envelope::SocketReply => SocketReply::is_valid(&value),
        Envelope::SocketDiscovery => SocketDiscovery::is_valid(&value),
        Envelope::PageRequest => PageRequest::is_valid(&value),
    }
}

pub(crate) fn hello(value: &serde_json::Value) -> bool {
    SocketHello::is_valid(value)
}
pub(crate) fn hello_success(input: &str) -> bool {
    serde_json::from_str(input).is_ok_and(|value| SocketHelloSuccess::is_valid(&value))
}
