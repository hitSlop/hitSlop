//! Shared page admission and replies; native socket routing lives in `native`.
use crate::owner;
use crate::owner::{Failure, FailureKind, Owner};
use pop_contracts::app::Capability;
use pop_contracts::wire;
use pop_contracts::wire::{Code, OutcomeCode, PageRequest};
use serde_json::value::RawValue;
mod native;
pub use native::*;
pub type Result<T> = std::result::Result<T, Failure>;
fn invalid(message: impl Into<String>) -> Failure {
    Failure::rejected(Code::InvalidRequest, message)
}

/// A reply refusing with `error`: what its kind means for the caller. `accepted` once the
/// owner took a mutation, whose outcome a later failure leaves unknown; `saving` while a
/// save it waits for can fail.
pub fn failure(error: Failure, accepted: bool, saving: bool) -> String {
    let code = match error.kind {
        FailureKind::Full | FailureKind::Busy | FailureKind::Moved | FailureKind::SaveFailed if saving => {
            OutcomeCode::SaveFailed
        }
        FailureKind::SaveFailed => OutcomeCode::SaveFailed,
        _ if accepted => OutcomeCode::UnknownOutcome,
        FailureKind::Rejected | FailureKind::ReadOnly => OutcomeCode::Rejected,
        FailureKind::Replaced => OutcomeCode::OwnerReplaced,
        FailureKind::Closing | FailureKind::Closed => OutcomeCode::Closing,
        FailureKind::Invalidated => OutcomeCode::OwnerInvalidated,
        _ => OutcomeCode::UnknownOutcome,
    };
    let message = if accepted && code == OutcomeCode::UnknownOutcome {
        "Command was accepted, but its final state could not be confirmed.".into()
    } else {
        error.message
    };
    crate::encode(&wire::engine::Failure {
        ok: wire::engine::False,
        error: message,
        code,
        reason: (code == OutcomeCode::Rejected).then_some(error.reason).flatten(),
        op_index: (code == OutcomeCode::Rejected).then_some(error.op_index).flatten(),
    })
}
fn raw(value: String) -> Result<Box<RawValue>> {
    RawValue::from_string(value).map_err(|e| Failure::new(FailureKind::Failed, e.to_string()))
}
/// A page request is either answered by the owner or delivered as a typed host action.
/// Only attachment persistence failures are also surfaced to the native storage UI.
pub enum PageDispatch {
    Reply { json: String, failure: Option<Failure>, storage: bool },
    Host { action: wire::page::HostAction },
}
fn page_failure(error: Failure, storage: bool) -> PageDispatch {
    PageDispatch::Reply { json: failure(error.clone(), false, true), failure: Some(error), storage }
}
fn page_success(result: wire::page::PageSuccess) -> PageDispatch {
    PageDispatch::Reply { json: crate::encode(&result), failure: None, storage: false }
}
/// The page receives the original descriptor and authored geometry; native UI receives
/// the normalized window through UniFFI. Neither side reparses the stored definition.
fn page_config(owner: &Owner) -> Result<wire::page::PageSuccess> {
    let app = owner.app();
    Ok(wire::page::PageSuccess::Config {
        ok: wire::engine::True,
        runtime_abi: app.runtime_abi,
        style: app.style,
        window: app.app.window(),
        descriptor: raw(app.app.document_json().into())?,
    })
}
fn not_declared(capability: Capability) -> PageDispatch {
    use wire::page::{PageSuccess, SpeechOutcome, SpeechUnavailable};
    match capability {
        Capability::Speech => page_success(PageSuccess::SpeechTranscribe {
            ok: wire::engine::True,
            outcome: SpeechOutcome::Unavailable {
                reason: SpeechUnavailable::NotDeclared,
                message: r#"This app does not declare speech; add "speech" to capabilities in pop.ts"#.into(),
            },
        }),
        // Camera and microphone are admitted by the host delegate, never page requests.
        Capability::Camera | Capability::Microphone => {
            page_failure(invalid("This app does not declare the required capability"), false)
        }
    }
}

fn admit_page_dispatch(
    owner: &Owner,
    view: String,
    payload_bytes: usize,
    prepare: impl FnOnce() -> Result<PageDispatch>,
    reply: impl FnOnce(PageDispatch) + Send + 'static,
) {
    let permit = match owner.reserve_page(view.capacity(), payload_bytes) {
        Ok(permit) => permit,
        Err(error) => return reply(page_failure(error, false)),
    };
    let dispatch = prepare();
    owner.admit_page(
        view,
        permit,
        Box::new(move |admitted| {
            reply(match admitted.and(dispatch) {
                Ok(dispatch) => dispatch,
                Err(error) => page_failure(error, false),
            })
        }),
    );
}
/// Runs one validated request from `view`. Edits answer on acceptance; flush waits for
/// persistence. Host actions pass the same serial view/lifecycle fence before delivery.
pub fn page(owner: &Owner, view: String, input: &str, reply: impl FnOnce(PageDispatch) + Send + 'static) {
    match PageRequest::decode(input) {
        Ok(request) => page_validated(owner, view, request, reply),
        Err(error) => reply(page_failure(error.into(), false)),
    }
}

/// Trusted transports that have already decoded their envelope still use all page
/// bounds and the serial owner fence, without encoding JSON just to parse it again.
pub fn page_typed(
    owner: &Owner,
    view: String,
    request: PageRequest,
    reply: impl FnOnce(PageDispatch) + Send + 'static,
) {
    match request.check() {
        Ok(()) => page_validated(owner, view, request, reply),
        Err(error) => reply(page_failure(error.into(), false)),
    }
}

fn page_validated(
    owner: &Owner,
    view: String,
    request: PageRequest,
    reply: impl FnOnce(PageDispatch) + Send + 'static,
) {
    use owner::{Reply, Request};
    use wire::engine::True;
    use wire::page::{HostAction, PageSuccess as Success};
    type Answer = Box<dyn FnOnce(Reply) -> Option<Success> + Send>;
    if let Some(capability) = request.requires()
        && !owner.app().app.capabilities().contains(&capability)
    {
        let bytes = owner::serialized_size(&request).saturating_mul(2).saturating_add(512);
        admit_page_dispatch(owner, view, bytes, || Ok(not_declared(capability)), reply);
        return;
    }
    let storage = matches!(request, PageRequest::AttachmentsPut { .. });
    let (request, answer): (Request, Answer) = match request {
        PageRequest::Open {} => (
            Request::State,
            Box::new(|reply| match reply {
                Reply::State { reading, sequence } => {
                    Some(Success::Open { ok: True, state: crate::encode(&reading.sequenced(sequence)) })
                }
                _ => None,
            }),
        ),
        PageRequest::Apply { batch } => (
            Request::Apply { batch, origin: pop_core::Origin::Page },
            Box::new(|reply| match reply {
                Reply::Applied { sequence, ids, text } => Some(Success::Apply {
                    ok: True,
                    sequence,
                    ids,
                    selection_start: text.as_ref().map(|t| t.selection[0]),
                    selection_end: text.as_ref().map(|t| t.selection[1]),
                }),
                _ => None,
            }),
        ),
        PageRequest::Flush {} => {
            (Request::Flush, Box::new(|reply| matches!(reply, Reply::Unit).then_some(Success::Flush { ok: True })))
        }
        PageRequest::Undo {} => (
            Request::Undo { redo: false },
            Box::new(|reply| match reply {
                Reply::Applied { sequence, .. } => Some(Success::Undo { ok: True, sequence }),
                _ => None,
            }),
        ),
        PageRequest::Redo {} => (
            Request::Undo { redo: true },
            Box::new(|reply| match reply {
                Reply::Applied { sequence, .. } => Some(Success::Redo { ok: True, sequence }),
                _ => None,
            }),
        ),
        PageRequest::AttachmentsPut { bytes } => match data_encoding::BASE64.decode(bytes.as_bytes()) {
            Ok(bytes) => (
                Request::PutAttachment { bytes },
                Box::new(|reply| match reply {
                    Reply::Attachment { item } => Some(Success::AttachmentsPut {
                        ok: True,
                        id: item.id,
                        byte_length: item.bytes,
                        mime_type: item.media_type,
                    }),
                    _ => None,
                }),
            ),
            Err(_) => return reply(page_failure(invalid("Invalid attachment bytes"), false)),
        },
        request => {
            // serde's growing output buffer can retain up to twice its encoded size.
            // Config carries the accepted descriptor, not just the tiny input request;
            // charge it before cloning or serializing any response data. The allowance
            // covers the fixed envelope, scalar fields, and inline dispatch storage.
            let bytes = if matches!(&request, PageRequest::Config {}) {
                owner
                    .app()
                    .app
                    .document_json()
                    .len()
                    .saturating_add(owner::serialized_size(&owner.app().app.window()))
                    .saturating_add(512)
                    .saturating_mul(2)
            } else {
                owner::serialized_size(&request).saturating_mul(2).saturating_add(512)
            };
            admit_page_dispatch(
                owner,
                view,
                bytes,
                || match request {
                    PageRequest::Config {} => page_config(owner).map(page_success),
                    request => {
                        let mut action = request.into_host().expect("document requests routed above");
                        if let HostAction::SpeechTranscribe { locale: Some(locale), .. } = &mut action {
                            *locale = locale.replace('_', "-");
                        }
                        Ok(PageDispatch::Host { action })
                    }
                },
                reply,
            );
            return;
        }
    };
    owner.submit(
        request,
        Some(view),
        Box::new(move |result| {
            reply(match result.map(answer) {
                Ok(Some(result)) => page_success(result),
                Ok(None) => page_failure(Failure::new(FailureKind::Invalidated, "Unexpected owner result"), storage),
                Err(error) => page_failure(error, storage),
            })
        }),
    );
}
pub fn protocol() -> String {
    serde_json::json!({ "version": wire::ENGINE_PROTOCOL }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accepted_mutation_failure_does_not_claim_it_was_refused() {
        let refusal = "Document changed owners; the request was not applied";
        let result: serde_json::Value =
            serde_json::from_str(&failure(Failure::new(FailureKind::Replaced, refusal), true, false)).unwrap();
        assert_eq!(result["code"], "unknown_outcome");
        assert!(result["error"].as_str().unwrap().contains("could not be confirmed"));
        assert!(!result["error"].as_str().unwrap().contains("not applied"));
        let refused: serde_json::Value =
            serde_json::from_str(&failure(Failure::new(FailureKind::Replaced, refusal), false, false)).unwrap();
        assert_eq!(refused["code"], "owner_replaced");
        assert_eq!(refused["error"], refusal);
        assert!(refused.get("reason").is_none());
        assert!(refused.get("opIndex").is_none());

        let rejected = Failure {
            kind: FailureKind::Rejected,
            message: "Invalid edit".into(),
            reason: Some(Code::InvalidRequest),
            op_index: Some(2),
            path: vec![],
        };
        assert_eq!(
            failure(rejected.clone(), false, false),
            r#"{"ok":false,"error":"Invalid edit","code":"rejected","reason":"invalid_request","opIndex":2}"#
        );
        let accepted: serde_json::Value = serde_json::from_str(&failure(rejected, true, false)).unwrap();
        assert_eq!(accepted["code"], "unknown_outcome");
        assert!(accepted.get("reason").is_none());
        assert!(accepted.get("opIndex").is_none());
    }

    #[test]
    fn an_accepted_mutation_save_failure_keeps_its_classification_and_message() {
        let result: serde_json::Value =
            serde_json::from_str(&failure(Failure::new(FailureKind::Full, "Document storage is full"), true, true))
                .unwrap();
        assert_eq!(result["code"], "save_failed");
        assert_eq!(result["error"], "Document storage is full");
    }
}
