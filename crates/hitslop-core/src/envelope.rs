//! Native acceptance of platform envelopes over the TypeBox-generated contracts: socket
//! requests, replies and discovery files, and page bridge requests. The schemas are the
//! only rules; this module parses bounded JSON and answers whether an envelope matches.

#[jsonschema::validator(
    path = "../../packages/schema/generated/socket-request.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = true, validate = false, iter_errors = false }
)]
struct SocketRequest;

#[jsonschema::validator(
    path = "../../packages/schema/generated/socket-reply.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = true, validate = false, iter_errors = false }
)]
struct SocketReply;

#[jsonschema::validator(
    path = "../../packages/schema/generated/socket-discovery.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = true, validate = false, iter_errors = false }
)]
struct SocketDiscovery;

#[jsonschema::validator(
    path = "../../packages/schema/generated/bridge.schema.json",
    draft = Draft7,
    validate_formats = true,
    methods = { is_valid = true, validate = false, iter_errors = false }
)]
struct BridgeRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Envelope {
    SocketRequest,
    SocketReply,
    SocketDiscovery,
    BridgeRequest,
}

/// The largest envelope is an attachment upload (16 MiB on the socket, less on the
/// bridge); anything bigger is refused before parsing.
const MAX_BYTES: usize = 48 * 1024 * 1024;

pub fn is_valid(kind: Envelope, json: &[u8]) -> bool {
    if json.len() > MAX_BYTES {
        return false;
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(json) else {
        return false;
    };
    match kind {
        Envelope::SocketRequest => SocketRequest::is_valid(&value),
        Envelope::SocketReply => SocketReply::is_valid(&value),
        Envelope::SocketDiscovery => SocketDiscovery::is_valid(&value),
        Envelope::BridgeRequest => BridgeRequest::is_valid(&value),
    }
}
