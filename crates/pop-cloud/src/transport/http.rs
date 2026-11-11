//! One typed HTTP operation controls routing, response bounds and refusal semantics.
use super::*;
#[derive(Clone, Copy)]
pub(crate) enum Endpoint<'a> {
    Documents,
    Account,
    Library,
    Document(&'a str),
    Sharing(&'a str),
    Members(&'a str),
    Member(&'a str),
    Seed(&'a str),
    Blobs(&'a str),
    Blob(&'a str, &'a str),
    Open(&'a str),
    Compact(&'a str),
    Sync(&'a str),
}
impl Endpoint<'_> {
    pub(super) fn url(self, origin: &str) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(origin).map_err(|e| Error::caused(PauseKind::Failed, e.to_string(), e))?;
        let mut path = url.path_segments_mut().map_err(|_| Error::from("Invalid sharing origin"))?;
        path.clear().push("v1");
        match self {
            Self::Documents => {
                path.push("documents");
            }
            Self::Account => {
                path.push("account");
            }
            Self::Library => {
                path.extend(["account", "library"]);
            }
            endpoint => {
                let (document, suffix) = match endpoint {
                    Self::Document(d) => (d, None),
                    Self::Sharing(d) => (d, Some("sharing")),
                    Self::Members(d) => (d, Some("members")),
                    Self::Member(d) => (d, Some("members")),
                    Self::Seed(d) => (d, Some("seed")),
                    Self::Blobs(d) => (d, Some("blobs")),
                    Self::Blob(d, _) => (d, Some("blob")),
                    Self::Open(d) => (d, Some("open")),
                    Self::Compact(d) => (d, Some("compact")),
                    Self::Sync(d) => (d, Some("sync")),
                    _ => unreachable!("account endpoints handled above"),
                };
                path.extend(["documents", document]);
                if let Some(suffix) = suffix {
                    path.push(suffix);
                }
                if let Self::Member(_) = endpoint {
                    path.push("me");
                }
                if let Self::Blob(_, hash) = endpoint {
                    path.push(hash);
                }
            }
        }
        drop(path);
        Ok(url)
    }
    pub(super) fn classification(self) -> Route {
        match self {
            Self::Compact(_) => Route::Compact,
            Self::Blob(..) => Route::Blob,
            _ => Route::Other,
        }
    }
    fn limit(self) -> u64 {
        match self {
            Self::Seed(_) => pop_sharing_protocol::SEED_BYTES as u64,
            Self::Blob(..) => pop_contracts::wire::ATTACHMENT_FILE_BYTES as u64,
            _ => pop_sharing_protocol::CONTROL_BYTES as u64,
        }
    }
}
pub(crate) enum Reply<'a> {
    Bytes,
    Optional(&'a [u16]),
}
pub(crate) struct HttpOperation<'a> {
    method: reqwest::Method,
    endpoint: Endpoint<'a>,
    body: &'a [u8],
    headers: &'a [(&'a str, &'a str)],
    reply: Reply<'a>,
}
impl<'a> HttpOperation<'a> {
    pub(crate) fn new(method: reqwest::Method, endpoint: Endpoint<'a>) -> Self {
        Self { method, endpoint, body: &[], headers: &[], reply: Reply::Bytes }
    }
    pub(crate) fn body(mut self, body: &'a [u8]) -> Self {
        self.body = body;
        self
    }
    pub(crate) fn headers(mut self, headers: &'a [(&'a str, &'a str)]) -> Self {
        self.headers = headers;
        self
    }
    pub(crate) fn reply(mut self, reply: Reply<'a>) -> Self {
        self.reply = reply;
        self
    }
}
pub(crate) async fn http(origin: &str, identity: &str, operation: HttpOperation<'_>) -> Result<Option<Vec<u8>>> {
    let url = operation.endpoint.url(origin)?;
    let mut request =
        network()?.http.request(operation.method.clone(), url).bearer_auth(identity).body(operation.body.to_vec());
    for (name, value) in operation.headers {
        request = request.header(*name, *value);
    }
    let response = request
        .send()
        .await
        .map_err(|e| Error::caused(PauseKind::Failed, "Sharing service is unavailable".into(), e))?;
    let status = response.status().as_u16();
    if let Reply::Optional(absent) = operation.reply {
        // Absence can authorize local removal or creation. Preserve a failed or
        // oversized lookup body as an error before accepting that authority.
        let bytes = response_body(response, pop_sharing_protocol::CONTROL_BYTES as u64).await?;
        return match status {
            200 => Ok(Some(bytes)),
            status if absent.contains(&status) => Ok(None),
            _ => Err(refusal(status, &bytes, operation.endpoint.classification())),
        };
    }
    if !matches!(status, 200 | 204) {
        let bytes = response_body(response, pop_sharing_protocol::CONTROL_BYTES as u64).await.unwrap_or_default();
        if status == 410 && operation.method == reqwest::Method::DELETE {
            return Ok(Some(Vec::new()));
        }
        return Err(refusal(status, &bytes, operation.endpoint.classification()));
    }
    response_body(response, operation.endpoint.limit()).await.map(Some)
}
