//! Firebase identity verification, independent of room permissions. Only Google's
//! fixed certificate endpoint supplies keys; JWT headers never select a remote URL.
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use std::collections::BTreeMap;

const TOKEN_BYTES: usize = tinys_sharing_protocol::limits::IDENTITY_TOKEN_BYTES;
#[derive(Clone, Deserialize)]
pub(crate) struct Claims {
    pub sub: String,
    pub firebase: Firebase,
    pub exp: u64,
    pub iat: u64,
    pub auth_time: u64,
}
#[derive(Clone, Deserialize)]
pub(crate) struct Firebase {
    sign_in_provider: String,
}
pub(crate) fn key_id(token: &str) -> Result<String, &'static str> {
    if token.len() > TOKEN_BYTES {
        return Err("Invalid identity");
    }
    let header = decode_header(token).map_err(|_| "Invalid identity")?;
    if header.alg != Algorithm::RS256 {
        return Err("Invalid identity");
    }
    header.kid.filter(|id| !id.is_empty() && id.len() <= 128).ok_or("Invalid identity")
}
pub(crate) fn verify(token: &str, project: &str, key: &DecodingKey, now: u64) -> Result<Claims, &'static str> {
    key_id(token)?;
    let mut policy = Validation::new(Algorithm::RS256);
    policy.set_audience(&[project]);
    policy.set_issuer(&[format!("https://securetoken.google.com/{project}")]);
    policy.set_required_spec_claims(&["exp", "iss", "aud", "sub", "iat"]);
    // Use the caller's explicit clock for native tests and the same check on Workers.
    policy.validate_exp = false;
    policy.leeway = 0;
    let claims = decode::<Claims>(token, key, &policy).map_err(|_| "Invalid identity")?.claims;
    // Any Firebase provider may own links (Apple today; Google or others later), but an
    // anonymous account is free to mint and must not reach the room quota.
    if claims.firebase.sign_in_provider == "anonymous"
        || claims.sub.is_empty()
        || claims.sub.chars().count() > 128
        || claims.exp <= now
        || claims.iat > now
        || claims.auth_time > now
        || claims.auth_time > claims.iat
    {
        return Err("Invalid identity");
    }
    Ok(claims)
}

struct Keys {
    values: BTreeMap<String, DecodingKey>,
    fetched: u64,
    expires: u64,
}
impl Keys {
    /// None requires refresh, Err refuses an unknown ID during the refresh cooldown.
    fn cached(&self, id: &str, now: u64) -> Option<Result<DecodingKey, ()>> {
        if now < self.expires {
            if let Some(key) = self.values.get(id) {
                return Some(Ok(key.clone()));
            }
            if now < self.fetched.saturating_add(30) {
                return Some(Err(()));
            }
        }
        None
    }
}

#[cfg(target_arch = "wasm32")]
mod runtime {
    use super::*;
    use std::cell::RefCell;
    use worker::*;
    #[derive(Debug)]
    pub(crate) enum IdentityError {
        Denied,
        Unavailable,
    }
    type IdentityResult<T> = std::result::Result<T, IdentityError>;
    const CERTIFICATES: &str =
        "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com";
    thread_local! { static KEYS: RefCell<Option<Keys>> = const { RefCell::new(None) }; }
    async fn signing_key(id: &str, now: u64) -> IdentityResult<DecodingKey> {
        if let Some(cached) = KEYS.with(|cache| {
            let cache = cache.borrow();
            cache.as_ref()?.cached(id, now)
        }) {
            return cached.map_err(|_| IdentityError::Denied);
        }
        let mut response = Fetch::Url(Url::parse(CERTIFICATES).map_err(|_| IdentityError::Unavailable)?)
            .send()
            .await
            .map_err(|_| IdentityError::Unavailable)?;
        if response.status_code() != 200 {
            return Err(IdentityError::Unavailable);
        }
        let max_age = response
            .headers()
            .get("Cache-Control")
            .map_err(|_| IdentityError::Unavailable)?
            .unwrap_or_default()
            .split(',')
            .find_map(|part| part.trim().strip_prefix("max-age=")?.parse::<u64>().ok())
            .unwrap_or(0)
            .min(86_400);
        let bytes = response.bytes().await.map_err(|_| IdentityError::Unavailable)?;
        if bytes.len() > 256 * 1024 {
            return Err(IdentityError::Unavailable);
        }
        let certificates: BTreeMap<String, String> =
            serde_json::from_slice(&bytes).map_err(|_| IdentityError::Unavailable)?;
        let values: BTreeMap<_, _> = certificates
            .into_iter()
            .map(|(id, pem)| {
                DecodingKey::from_rsa_pem(pem.as_bytes()).map(|key| (id, key)).map_err(|_| IdentityError::Unavailable)
            })
            .collect::<IdentityResult<_>>()?;
        let selected = values.get(id).cloned();
        KEYS.with(|cache| {
            *cache.borrow_mut() = Some(Keys { values, fetched: now, expires: now.saturating_add(max_age) })
        });
        selected.ok_or(IdentityError::Denied)
    }
    pub(crate) async fn identity(req: &Request, env: &Env) -> IdentityResult<Claims> {
        let header = req.headers().get("Authorization").map_err(|_| IdentityError::Denied)?.unwrap_or_default();
        let token = header.strip_prefix("Bearer ").ok_or(IdentityError::Denied)?;
        let id = key_id(token).map_err(|_| IdentityError::Denied)?;
        let project = env.var("FIREBASE_PROJECT").map_err(|_| IdentityError::Unavailable)?.to_string();
        let now = (js_sys::Date::now() / 1000.) as u64;
        let key = signing_key(&id, now).await?;
        verify(token, &project, &key, now).map_err(|_| IdentityError::Denied)
    }
}
#[cfg(target_arch = "wasm32")]
pub(crate) use runtime::{IdentityError, identity};

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use rand::SeedableRng;
    use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
    use serde_json::{Value, json};
    use std::sync::OnceLock;
    const NOW: u64 = 1_700_000_000;
    fn keys() -> &'static (EncodingKey, DecodingKey) {
        static KEYS: OnceLock<(EncodingKey, DecodingKey)> = OnceLock::new();
        KEYS.get_or_init(|| {
            // Deterministic test-only identity; no saved private keys or Firebase users.
            let mut rng = rand::rngs::StdRng::seed_from_u64(753);
            let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
            (
                EncodingKey::from_rsa_der(key.to_pkcs1_der().unwrap().as_bytes()),
                DecodingKey::from_rsa_der(key.to_public_key().to_pkcs1_der().unwrap().as_bytes()),
            )
        })
    }
    fn claims() -> Value {
        json!({"sub":"test-user","aud":"hitslopapp","iss":"https://securetoken.google.com/hitslopapp",
            "firebase":{"sign_in_provider":"apple.com"},"exp":NOW+3600,"iat":NOW,"auth_time":NOW-10})
    }
    fn token(claims: &Value, kid: &str) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.into());
        encode(&header, claims, &keys().0).unwrap()
    }
    #[test]
    fn firebase_identity_requires_signature_project_times_and_nonempty_subject() {
        let signed = token(&claims(), "current");
        assert_eq!(key_id(&signed).unwrap(), "current");
        assert_eq!(verify(&signed, "hitslopapp", &keys().1, NOW).unwrap().sub, "test-user");
        assert!(verify(&signed, "another-project", &keys().1, NOW).is_err());
        assert!(verify(&signed, "hitslopapp", &keys().1, NOW + 3600).is_err());
        for (field, value) in [
            ("sub", json!("")),
            ("sub", json!("x".repeat(129))),
            ("aud", json!("other")),
            ("iss", json!("https://attacker.example")),
            ("iat", json!(NOW + 1)),
            ("auth_time", json!(NOW + 1)),
            ("auth_time", json!("invalid")),
            ("exp", json!(NOW)),
        ] {
            let mut changed = claims();
            changed[field] = value;
            assert!(verify(&token(&changed, "current"), "hitslopapp", &keys().1, NOW).is_err(), "{field}");
        }
        for field in ["sub", "aud", "iss", "exp", "iat", "auth_time"] {
            let mut changed = claims();
            changed.as_object_mut().unwrap().remove(field);
            assert!(verify(&token(&changed, "current"), "hitslopapp", &keys().1, NOW).is_err(), "{field}");
        }
        let mut forged: Vec<_> = signed.split('.').map(str::to_owned).collect();
        // Replace only the payload of a genuinely signed token.
        let mut changed = claims();
        changed["sub"] = json!("other-user");
        forged[1] = token(&changed, "current").split('.').nth(1).unwrap().into();
        assert!(verify(&forged.join("."), "hitslopapp", &keys().1, NOW).is_err());
    }
    #[test]
    fn owner_identity_accepts_named_providers_but_not_anonymous_accounts() {
        for provider in ["apple.com", "google.com", "password"] {
            let mut changed = claims();
            changed["firebase"]["sign_in_provider"] = json!(provider);
            assert!(verify(&token(&changed, "current"), "hitslopapp", &keys().1, NOW).is_ok());
        }
        let mut changed = claims();
        changed["firebase"]["sign_in_provider"] = json!("anonymous");
        assert!(verify(&token(&changed, "current"), "hitslopapp", &keys().1, NOW).is_err());
        let mut changed = claims();
        changed.as_object_mut().unwrap().remove("firebase");
        assert!(verify(&token(&changed, "current"), "hitslopapp", &keys().1, NOW).is_err());
    }
    #[test]
    fn firebase_identity_refuses_algorithm_confusion_and_missing_key_ids() {
        let hmac = encode(&Header::new(Algorithm::HS256), &claims(), &EncodingKey::from_secret(b"test")).unwrap();
        assert!(key_id(&hmac).is_err());
        assert!(verify(&hmac, "hitslopapp", &keys().1, NOW).is_err());
        assert!(key_id(&token(&claims(), "")).is_err());
        assert!(key_id(&"x".repeat(TOKEN_BYTES + 1)).is_err());
        assert!(key_id("malformed").is_err());
    }
    #[test]
    fn firebase_keys_refresh_on_rotation_and_expiry_without_unknown_id_fetch_storms() {
        let current =
            Keys { values: BTreeMap::from([("current".into(), keys().1.clone())]), fetched: NOW, expires: NOW + 3600 };
        assert!(current.cached("current", NOW).unwrap().is_ok());
        assert!(current.cached("rotated", NOW + 29).unwrap().is_err());
        assert!(current.cached("rotated", NOW + 30).is_none());
        assert!(current.cached("current", NOW + 3600).is_none());
        // A successful refresh replaces, rather than extends, the trusted key set.
        let refreshed = Keys {
            values: BTreeMap::from([("rotated".into(), keys().1.clone())]),
            fetched: NOW + 30,
            expires: NOW + 3630,
        };
        let signed = token(&claims(), "rotated");
        assert!(
            verify(&signed, "hitslopapp", &refreshed.cached("rotated", NOW + 30).unwrap().unwrap(), NOW + 30).is_ok()
        );
        assert!(refreshed.cached("current", NOW + 30).unwrap().is_err());
    }
}
