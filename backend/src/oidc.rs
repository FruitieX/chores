//! Optional sign-in through an OpenID Connect provider (Pocket ID): the
//! authorization code flow with PKCE. A successful sign-in links the
//! provider identity to one of the two local accounts and then issues the
//! ordinary Chores session.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

pub const STATE_COOKIE: &str = "chores_oidc";
pub const STATE_SECONDS: u64 = 600;

#[derive(Clone, Debug)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    /// Where the provider sends the browser back to (`…/auth/callback`).
    pub redirect_url: String,
    /// Sign-in needs membership in one of these groups; empty allows anyone the provider lets through.
    pub allowed_groups: Vec<String>,
    /// Provider username (lowercase) → local account (`a`, `b` or the account email).
    pub user_map: HashMap<String, String>,
}

impl OidcConfig {
    /// Reads `CHORES_OIDC_*`. Returns `Ok(None)` when sign-in through a provider is not configured.
    pub fn from_env(origin: &str) -> Result<Option<Self>, String> {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let (issuer, client_id, client_secret) = match (
            var("CHORES_OIDC_ISSUER"),
            var("CHORES_OIDC_CLIENT_ID"),
            var("CHORES_OIDC_CLIENT_SECRET"),
        ) {
            (None, None, None) => return Ok(None),
            (Some(i), Some(c), Some(s)) => (i, c, s),
            _ => {
                return Err("Set CHORES_OIDC_ISSUER, CHORES_OIDC_CLIENT_ID and CHORES_OIDC_CLIENT_SECRET together".into());
            }
        };
        Ok(Some(Self {
            issuer: issuer.trim_end_matches('/').to_string(),
            client_id,
            client_secret,
            redirect_url: format!("{origin}/auth/callback"),
            allowed_groups: list(&var("CHORES_OIDC_ALLOWED_GROUPS").unwrap_or_default()),
            user_map: parse_user_map(&var("CHORES_OIDC_USER_MAP").unwrap_or_default())?,
        }))
    }
}

fn list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
        .collect()
}

/// Parses `pocketuser:localaccount,…`, where the local account is `a`, `b` or its email.
pub fn parse_user_map(value: &str) -> Result<HashMap<String, String>, String> {
    list(value)
        .into_iter()
        .map(|pair| {
            let (user, account) = pair
                .split_once(':')
                .ok_or_else(|| format!("CHORES_OIDC_USER_MAP entry {pair:?} needs user:account"))?;
            let (user, account) = (user.trim().to_lowercase(), account.trim().to_lowercase());
            if user.is_empty() || account.is_empty() {
                return Err(format!(
                    "CHORES_OIDC_USER_MAP entry {pair:?} needs user:account"
                ));
            }
            Ok((user, account))
        })
        .collect()
}

#[derive(Clone, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
}

/// Login state remembered between `/auth/login` and the callback.
#[derive(Clone, Debug)]
pub struct LoginState {
    pub nonce: String,
    pub verifier: String,
    pub exp: u64,
}

/// Who signed in at the provider.
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub issuer: String,
    pub sub: String,
    pub username: Option<String>,
}

#[derive(Deserialize)]
struct IdClaims {
    iss: String,
    sub: String,
    aud: Audience,
    exp: u64,
    nonce: Option<String>,
    preferred_username: Option<String>,
    #[serde(default)]
    groups: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

impl Audience {
    fn contains(&self, id: &str) -> bool {
        match self {
            Audience::One(aud) => aud == id,
            Audience::Many(auds) => auds.iter().any(|aud| aud == id),
        }
    }
}

/// Why a sign-in was refused; the code is shown to the browser.
#[derive(Debug)]
pub enum Refusal {
    /// Not in an allowed group.
    NotAllowed(String),
    /// Anything else (bad state, provider error, bad claims).
    Failed(String),
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::NotAllowed(_) => "not-allowed",
            Refusal::Failed(_) => "failed",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::NotAllowed(m) | Refusal::Failed(m) => f.write_str(m),
        }
    }
}

fn failed(e: impl std::fmt::Display) -> Refusal {
    Refusal::Failed(e.to_string())
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn form_encode(pairs: &[(&str, &str)]) -> String {
    fn encode(s: &str) -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect()
    }
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub struct Oidc {
    pub config: OidcConfig,
    discovery: Mutex<Option<Discovery>>,
    logins: Mutex<HashMap<String, LoginState>>,
}

impl Oidc {
    pub fn new(config: OidcConfig) -> Self {
        Self {
            config,
            discovery: Mutex::new(None),
            logins: Mutex::new(HashMap::new()),
        }
    }

    /// Blocking: fetches the provider's metadata once and caches it.
    fn discovery(&self) -> Result<Discovery, Refusal> {
        if let Some(d) = self.discovery.lock().map_err(failed)?.clone() {
            return Ok(d);
        }
        let url = format!("{}/.well-known/openid-configuration", self.config.issuer);
        let discovery: Discovery = ureq::get(&url)
            .call()
            .map_err(|e| failed(format!("fetching {url}: {e}")))?
            .body_mut()
            .read_json()
            .map_err(failed)?;
        *self.discovery.lock().map_err(failed)? = Some(discovery.clone());
        Ok(discovery)
    }

    /// Blocking: the provider URL to send the browser to, and the state value for its cookie.
    pub fn start(&self) -> Result<(String, String), Refusal> {
        let discovery = self.discovery()?;
        let state = crate::token();
        let login = LoginState {
            nonce: crate::token(),
            verifier: crate::token(),
            exp: now() + STATE_SECONDS,
        };
        let query = form_encode(&[
            ("response_type", "code"),
            ("client_id", &self.config.client_id),
            ("redirect_uri", &self.config.redirect_url),
            ("scope", "openid profile email groups"),
            ("state", &state),
            ("nonce", &login.nonce),
            ("code_challenge", &pkce_challenge(&login.verifier)),
            ("code_challenge_method", "S256"),
        ]);
        {
            let mut logins = self.logins.lock().map_err(failed)?;
            let current = now();
            logins.retain(|_, l| l.exp > current);
            if logins.len() > 1000 {
                return Err(failed("too many pending sign-ins"));
            }
            logins.insert(state.clone(), login);
        }
        let separator = if discovery.authorization_endpoint.contains('?') {
            '&'
        } else {
            '?'
        };
        Ok((
            format!("{}{separator}{query}", discovery.authorization_endpoint),
            state,
        ))
    }

    /// Takes the pending login for `state`, if the browser's cookie names the same one.
    pub fn take_login(&self, state: &str, cookie: Option<&str>) -> Result<LoginState, Refusal> {
        if cookie != Some(state) {
            return Err(failed("sign-in state does not match this browser"));
        }
        self.logins
            .lock()
            .map_err(failed)?
            .remove(state)
            .filter(|l| l.exp > now())
            .ok_or_else(|| failed("missing or expired sign-in state"))
    }

    /// Blocking: redeems the authorization code and checks who signed in.
    pub fn finish(&self, login: &LoginState, code: &str) -> Result<Identity, Refusal> {
        let discovery = self.discovery()?;
        #[derive(Deserialize)]
        struct TokenResponse {
            id_token: String,
        }
        let body = form_encode(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.config.redirect_url),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
            ("code_verifier", &login.verifier),
        ]);
        let tokens: TokenResponse = ureq::post(&discovery.token_endpoint)
            .header("content-type", "application/x-www-form-urlencoded")
            .send(body)
            .map_err(|e| failed(format!("redeeming the authorization code: {e}")))?
            .body_mut()
            .read_json()
            .map_err(failed)?;
        self.check_id_token(&discovery.issuer, login, &tokens.id_token)
    }

    /// The ID token comes straight from the token endpoint over TLS, so its
    /// signature need not be checked (OpenID Connect Core 3.1.3.7); the
    /// claims still are.
    fn check_id_token(
        &self,
        issuer: &str,
        login: &LoginState,
        id_token: &str,
    ) -> Result<Identity, Refusal> {
        let payload = id_token
            .split('.')
            .nth(1)
            .ok_or_else(|| failed("malformed ID token"))?;
        let claims: IdClaims = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(payload.trim_end_matches('='))
                .map_err(failed)?,
        )
        .map_err(|e| failed(format!("reading ID token claims: {e}")))?;
        if claims.iss != issuer {
            return Err(failed(format!(
                "ID token from unexpected issuer {}",
                claims.iss
            )));
        }
        if !claims.aud.contains(&self.config.client_id) {
            return Err(failed("ID token is for another client"));
        }
        if claims.exp <= now() {
            return Err(failed("ID token has expired"));
        }
        if claims.nonce.as_deref() != Some(login.nonce.as_str()) {
            return Err(failed("ID token nonce does not match"));
        }
        let allowed = &self.config.allowed_groups;
        if !allowed.is_empty() && !claims.groups.iter().any(|g| allowed.contains(g)) {
            return Err(Refusal::NotAllowed(format!(
                "{} is not in an allowed group",
                claims.preferred_username.as_deref().unwrap_or(&claims.sub)
            )));
        }
        Ok(Identity {
            issuer: claims.iss,
            sub: claims.sub,
            username: claims.preferred_username,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn config(groups: &[&str]) -> OidcConfig {
        OidcConfig {
            issuer: "https://id.example.org".into(),
            client_id: "chores".into(),
            client_secret: "s".into(),
            redirect_url: "https://chores.example.org/auth/callback".into(),
            allowed_groups: groups.iter().map(|g| g.to_string()).collect(),
            user_map: parse_user_map("Rasse:a, julle:JULIA@example.org").unwrap(),
        }
    }

    fn login() -> LoginState {
        LoginState {
            nonce: "n1".into(),
            verifier: "v".into(),
            exp: now() + 60,
        }
    }

    fn token(claims: serde_json::Value) -> String {
        format!(
            "e30.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        )
    }

    fn claims() -> serde_json::Value {
        serde_json::json!({
            "iss": "https://id.example.org", "sub": "u1", "aud": "chores",
            "exp": now() + 60, "nonce": "n1", "preferred_username": "rasse", "groups": ["family"],
        })
    }

    #[test]
    fn accepts_valid_id_token() {
        let identity = Oidc::new(config(&["family"]))
            .check_id_token("https://id.example.org", &login(), &token(claims()))
            .unwrap();
        assert_eq!(
            identity,
            Identity {
                issuer: "https://id.example.org".into(),
                sub: "u1".into(),
                username: Some("rasse".into())
            }
        );
    }

    #[test]
    fn rejects_bad_claims() {
        let cases = [
            ("iss", serde_json::json!("https://evil.example.org")),
            ("aud", serde_json::json!(["other"])),
            ("exp", serde_json::json!(1)),
            ("nonce", serde_json::json!("n2")),
            ("groups", serde_json::json!(["kids"])),
        ];
        let oidc = Oidc::new(config(&["family"]));
        for (key, value) in cases {
            let mut claims = claims();
            claims[key] = value;
            let refusal = oidc
                .check_id_token("https://id.example.org", &login(), &token(claims))
                .expect_err(key);
            assert_eq!(
                refusal.code(),
                if key == "groups" {
                    "not-allowed"
                } else {
                    "failed"
                }
            );
        }
        let mut claims = claims();
        claims["groups"] = serde_json::json!([]);
        assert!(
            Oidc::new(config(&[]))
                .check_id_token("https://id.example.org", &login(), &token(claims))
                .is_ok()
        );
    }

    #[test]
    fn state_must_match_the_browser_and_is_single_use() {
        let oidc = Oidc::new(config(&[]));
        *oidc.discovery.lock().unwrap() = Some(Discovery {
            issuer: "https://id.example.org".into(),
            authorization_endpoint: "https://id.example.org/authorize".into(),
            token_endpoint: "https://id.example.org/api/oidc/token".into(),
        });
        let (url, state) = oidc.start().unwrap();
        assert!(url.starts_with("https://id.example.org/authorize?response_type=code&"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fchores.example.org%2Fauth%2Fcallback"));
        assert!(url.contains(&format!("state={state}")));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(oidc.take_login(&state, None).is_err());
        assert!(oidc.take_login(&state, Some("other")).is_err());
        let login = oidc.take_login(&state, Some(&state)).unwrap();
        assert!(url.contains(&format!(
            "code_challenge={}",
            pkce_challenge(&login.verifier)
        )));
        assert!(oidc.take_login(&state, Some(&state)).is_err());
    }

    #[test]
    fn user_map_parsing() {
        let map = config(&[]).user_map;
        assert_eq!(map["rasse"], "a");
        assert_eq!(map["julle"], "julia@example.org");
        assert!(parse_user_map("").unwrap().is_empty());
        assert!(parse_user_map("rasse").is_err());
        assert!(parse_user_map("rasse:").is_err());
    }
}
