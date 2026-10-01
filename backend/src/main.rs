mod domain;
mod error;
#[cfg(test)]
mod tests;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::Utc;
use domain::{Household, Operation, Settings};
use error::{ApiError, Result};
use rand::{RngCore, rngs::OsRng};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
struct Server {
    db: Arc<Mutex<Connection>>,
    origin: Arc<String>,
    limits: Arc<Mutex<HashMap<String, (u32, i64)>>>,
}
impl Server {
    fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.db.lock().map_err(ApiError::internal)
    }
    fn limit(&self, key: &str, max: u32) -> Result<()> {
        let now = Utc::now().timestamp();
        let mut map = self.limits.lock().map_err(ApiError::internal)?;
        map.retain(|_, v| v.1 > now);
        let item = map.entry(key.into()).or_insert((0, now + 300));
        item.0 += 1;
        if item.0 > max {
            Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many attempts. Try again in five minutes.".into(),
            ))
        } else {
            Ok(())
        }
    }
}
fn token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn email(value: &str) -> Result<String> {
    let v = value.trim().to_lowercase();
    if v.len() > 254 || !v.contains('@') || v.chars().any(char::is_whitespace) {
        Err(ApiError::bad("Enter a valid email address."))
    } else {
        Ok(v)
    }
}
fn password(value: &str) -> Result<()> {
    if value.chars().count() < 12 || value.len() > 256 {
        Err(ApiError::bad(
            "Passwords need at least 12 characters (up to 256 bytes).",
        ))
    } else {
        Ok(())
    }
}
static AUTH_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
async fn hash(value: String) -> Result<String> {
    password(&value)?;
    let _permit = AUTH_SLOTS.acquire().await.map_err(ApiError::internal)?;
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(value.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|h| h.to_string())
            .map_err(ApiError::internal)
    })
    .await
    .map_err(ApiError::internal)?
}
async fn verify(value: String, hashed: String) -> Result<bool> {
    if value.len() > 256 {
        return Ok(false);
    }
    let _permit = AUTH_SLOTS.acquire().await.map_err(ApiError::internal)?;
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hashed).is_ok_and(|h| {
            Argon2::default()
                .verify_password(value.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .map_err(ApiError::internal)
}
fn initialize(path: &std::path::Path) -> Result<Connection> {
    let db = Connection::open(path)?;
    db.busy_timeout(std::time::Duration::from_secs(10))?;
    let mode = std::env::var("CHORES_SQLITE_JOURNAL_MODE").unwrap_or("WAL".into());
    if !["WAL", "DELETE"].contains(&mode.as_str()) {
        return Err(ApiError::bad("Unsupported journal mode."));
    }
    db.pragma_update(None, "journal_mode", mode)?;
    db.pragma_update(None, "synchronous", "EXTRA")?;
    db.pragma_update(None, "foreign_keys", true)?;
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > 1 {
        return Err(ApiError::bad("This database needs a newer Chores release."));
    }
    db.execute_batch("BEGIN IMMEDIATE;
      CREATE TABLE IF NOT EXISTS household(id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY CHECK(id IN ('a','b')), email TEXT UNIQUE NOT NULL, password TEXT NOT NULL, recovery TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS sessions(hash TEXT PRIMARY KEY, person TEXT NOT NULL REFERENCES users(id), expires INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS operations(id TEXT PRIMARY KEY,person TEXT NOT NULL,payload TEXT NOT NULL);
      PRAGMA user_version=1;COMMIT;")?;
    Ok(db)
}
fn read(db: &Connection) -> Result<Household> {
    let raw: String = db.query_row("SELECT state FROM household WHERE id=1", [], |r| r.get(0))?;
    Ok(serde_json::from_str(&raw)?)
}
fn write(db: &Connection, h: &Household) -> Result<()> {
    db.execute(
        "UPDATE household SET state=?1 WHERE id=1",
        [serde_json::to_string(h)?],
    )?;
    Ok(())
}
fn actor(server: &Server, headers: &HeaderMap) -> Result<String> {
    let raw = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .map(str::trim)
                .find_map(|c| c.strip_prefix("chores_session="))
        })
        .filter(|s| s.len() == 64)
        .ok_or_else(ApiError::unauthorized)?;
    server
        .conn()?
        .query_row(
            "SELECT person FROM sessions WHERE hash=?1 AND expires>?2",
            params![digest(raw), Utc::now().timestamp()],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(ApiError::unauthorized)
}
fn sign_in(server: &Server, person: &str, recovery: Value) -> Result<(HeaderMap, Json<Value>)> {
    let raw = token();
    let now = Utc::now().timestamp();
    let db = server.conn()?;
    db.execute("DELETE FROM sessions WHERE expires<=?1", [now])?;
    db.execute(
        "INSERT INTO sessions VALUES(?1,?2,?3)",
        params![digest(&raw), person, now + 90 * 86400],
    )?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        format!(
            "chores_session={raw}; Path=/; HttpOnly; SameSite=Strict; Max-Age=7776000{}",
            if server.origin.starts_with("https://") {
                "; Secure"
            } else {
                ""
            }
        )
        .parse()
        .map_err(ApiError::internal)?,
    );
    Ok((
        headers,
        Json(json!({"person":person,"recoveryCodes":recovery})),
    ))
}
async fn guard(State(server): State<Server>, request: Request, next: Next) -> Response {
    if !["GET", "HEAD", "OPTIONS"].contains(&request.method().as_str())
        && request
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            != Some(server.origin.as_str())
    {
        return ApiError(
            StatusCode::FORBIDDEN,
            "Open Chores at its configured address before making changes.".into(),
        )
        .into_response();
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("referrer-policy", "same-origin".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    headers.insert("content-security-policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; manifest-src 'self'; worker-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'".parse().unwrap());
    response
}
async fn status(State(server): State<Server>, headers: HeaderMap) -> Result<Json<Value>> {
    let setup = server
        .conn()?
        .query_row("SELECT EXISTS(SELECT 1 FROM users)", [], |r| {
            r.get::<_, bool>(0)
        })?;
    Ok(Json(
        json!({"initialized":setup,"person":actor(&server,&headers).ok()}),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Setup {
    settings: Settings,
    emails: [String; 2],
    passwords: [String; 2],
}
async fn setup(
    State(server): State<Server>,
    Json(input): Json<Setup>,
) -> Result<(HeaderMap, Json<Value>)> {
    server.limit("setup", 10)?;
    if server
        .conn()?
        .query_row("SELECT EXISTS(SELECT 1 FROM users)", [], |r| {
            r.get::<_, bool>(0)
        })?
    {
        return Err(ApiError::bad(
            "This household has already been set up. Sign in.",
        ));
    }
    domain::valid_settings(&input.settings)?;
    let emails = [email(&input.emails[0])?, email(&input.emails[1])?];
    if emails[0] == emails[1] {
        return Err(ApiError::bad(
            "Use a separate email address for each account.",
        ));
    }
    let hashes = [
        hash(input.passwords[0].clone()).await?,
        hash(input.passwords[1].clone()).await?,
    ];
    let recoveries = [token(), token()];
    let h = Household::new(input.settings);
    {
        let mut db = server.conn()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if tx.query_row("SELECT EXISTS(SELECT 1 FROM users)", [], |r| {
            r.get::<_, bool>(0)
        })? {
            return Err(ApiError::conflict());
        }
        for (i, id) in ["a", "b"].iter().enumerate() {
            tx.execute(
                "INSERT INTO users VALUES(?1,?2,?3,?4)",
                params![id, emails[i], hashes[i], digest(&recoveries[i])],
            )?;
        }
        tx.execute(
            "INSERT INTO household VALUES(1,?1)",
            [serde_json::to_string(&h)?],
        )?;
        tx.commit()?;
    }
    sign_in(&server, "a", json!(recoveries))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Login {
    email: String,
    password: String,
}
async fn login(
    State(server): State<Server>,
    Json(input): Json<Login>,
) -> Result<(HeaderMap, Json<Value>)> {
    server.limit("login-global", 100)?;
    let email = email(&input.email)?;
    server.limit(&format!("login:{email}"), 15)?;
    let user = server
        .conn()?
        .query_row(
            "SELECT id,password FROM users WHERE email=?1",
            [email],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?;
    // Always perform an Argon2 verification so nonexistent accounts do not have a cheap timing path.
    let (id,hashed)=user.unwrap_or_else(||("".into(),"$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0c2FsdA$11QdSE2dBvxz6PbjWGqmVsJN+QMaoETdqha3HV4ht7U".into()));
    if !verify(input.password, hashed).await? || id.is_empty() {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Email or password did not match.".into(),
        ));
    }
    sign_in(&server, &id, Value::Null)
}
async fn logout(
    State(server): State<Server>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<Value>)> {
    actor(&server, &headers)?;
    if let Some(raw) = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .map(str::trim)
                .find_map(|c| c.strip_prefix("chores_session="))
        })
    {
        server
            .conn()?
            .execute("DELETE FROM sessions WHERE hash=?1", [digest(raw)])?;
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        "chores_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
            .parse()
            .unwrap(),
    );
    Ok((headers, Json(json!({"ok":true}))))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Recover {
    email: String,
    code: String,
    password: String,
}
async fn recover(
    State(server): State<Server>,
    Json(input): Json<Recover>,
) -> Result<(HeaderMap, Json<Value>)> {
    server.limit("recovery", 15)?;
    let email = email(&input.email)?;
    let id = server
        .conn()?
        .query_row(
            "SELECT id FROM users WHERE email=?1 AND recovery=?2",
            params![email, digest(input.code.trim())],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| ApiError::bad("Email or recovery code did not match."))?;
    let hashed = hash(input.password).await?;
    let code = token();
    {
        let mut db = server.conn()?;
        let tx = db.transaction()?;
        let changed = tx.execute(
            "UPDATE users SET password=?1,recovery=?2 WHERE id=?3 AND recovery=?4",
            params![hashed, digest(&code), id, digest(input.code.trim())],
        )?;
        if changed != 1 {
            return Err(ApiError::conflict());
        }
        tx.execute("DELETE FROM sessions WHERE person=?1", [&id])?;
        tx.commit()?;
    }
    sign_in(&server, &id, json!({"person":id,"code":code}))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PasswordChange {
    current: String,
    password: String,
}
async fn change_password(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<PasswordChange>,
) -> Result<(HeaderMap, Json<Value>)> {
    let id = actor(&server, &headers)?;
    server.limit(&format!("password:{id}"), 10)?;
    let previous: String =
        server
            .conn()?
            .query_row("SELECT password FROM users WHERE id=?1", [&id], |r| {
                r.get(0)
            })?;
    if !verify(input.current, previous.clone()).await? {
        return Err(ApiError::bad("Current password did not match."));
    }
    let hashed = hash(input.password).await?;
    let code = token();
    {
        let mut db = server.conn()?;
        let tx = db.transaction()?;
        if tx.execute(
            "UPDATE users SET password=?1,recovery=?2 WHERE id=?3 AND password=?4",
            params![hashed, digest(&code), id, previous],
        )? != 1
        {
            return Err(ApiError::conflict());
        }
        tx.execute("DELETE FROM sessions WHERE person=?1", [&id])?;
        tx.commit()?;
    }
    sign_in(&server, &id, json!({"person":id,"code":code}))
}
fn snapshot(server: &Server, person: String) -> Result<Value> {
    let db = server.conn()?;
    let h = read(&db)?;
    let today = Utc::now()
        .with_timezone(&chrono_tz::Europe::Helsinki)
        .date_naive()
        .to_string();
    Ok(json!({"person":person,"household":h,"today":today}))
}
async fn state(State(server): State<Server>, headers: HeaderMap) -> Result<Json<Value>> {
    let person = actor(&server, &headers)?;
    Ok(Json(snapshot(&server, person)?))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Mutation {
    id: String,
    revision: u64,
    operation: Operation,
}
fn mutate_db(
    db: &mut Connection,
    person: &str,
    input: Mutation,
    today: chrono::NaiveDate,
) -> Result<()> {
    if input.id.len() > 80 || input.id.len() < 16 {
        return Err(ApiError::bad("Invalid request identifier."));
    }
    let payload = serde_json::to_string(&input.operation)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if let Some((old_person, old_payload)) = tx
        .query_row(
            "SELECT person,payload FROM operations WHERE id=?1",
            [&input.id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
    {
        if old_person != person || old_payload != payload {
            return Err(ApiError::conflict());
        }
        return Ok(());
    }
    let mut h = read(&tx)?;
    if h.revision != input.revision
        && !matches!(
            &input.operation,
            Operation::Complete { .. } | Operation::Undo { .. }
        )
    {
        return Err(ApiError::conflict());
    }
    h.apply(input.operation, person, today, &Utc::now().to_rfc3339())?;
    write(&tx, &h)?;
    tx.execute(
        "INSERT INTO operations VALUES(?1,?2,?3)",
        params![input.id, person, payload],
    )?;
    tx.commit()?;
    Ok(())
}
async fn mutate(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<Mutation>,
) -> Result<Json<Value>> {
    let person = actor(&server, &headers)?;
    server.limit(&format!("mutation:{person}"), 120)?;
    mutate_db(
        &mut *server.conn()?,
        &person,
        input,
        Utc::now()
            .with_timezone(&chrono_tz::Europe::Helsinki)
            .date_naive(),
    )?;
    Ok(Json(snapshot(&server, person)?))
}
async fn backup(
    State(server): State<Server>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<Value>)> {
    actor(&server, &headers)?;
    let h = read(&*server.conn()?)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=chores-backup.json".parse().unwrap(),
    );
    Ok((headers, Json(json!({"version":1,"household":h}))))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Restore {
    revision: u64,
    version: u32,
    household: Household,
}
async fn restore(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(mut input): Json<Restore>,
) -> Result<Json<Value>> {
    let person = actor(&server, &headers)?;
    if person != "a" {
        return Err(ApiError::forbidden());
    }
    if input.version != 1 {
        return Err(ApiError::bad("Unsupported backup version."));
    }
    input.household.validate()?;
    {
        let mut db = server.conn()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing = read(&tx)?;
        if existing.revision != input.revision {
            return Err(ApiError::conflict());
        }
        input.household.revision = existing.revision + 1;
        write(&tx, &input.household)?;
        tx.commit()?;
    }
    Ok(Json(snapshot(&server, person)?))
}
async fn ready(State(server): State<Server>) -> Result<Json<Value>> {
    server
        .conn()?
        .query_row("SELECT 1", [], |r| r.get::<_, i64>(0))?;
    Ok(Json(json!({"status":"ok"})))
}
fn app(server: Server, static_dir: PathBuf) -> Router {
    Router::new()
        .route(
            "/api/health",
            get(|| async { Json(json!({"status":"ok"})) }),
        )
        .route("/api/ready", get(ready))
        .route("/api/auth/status", get(status))
        .route("/api/auth/setup", post(setup))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/recover", post(recover))
        .route("/api/auth/password", post(change_password))
        .route("/api/state", get(state))
        .route("/api/operations", post(mutate))
        .route("/api/backup", get(backup))
        .route("/api/restore", post(restore))
        .fallback_service(
            ServeDir::new(&static_dir)
                .not_found_service(ServeFile::new(static_dir.join("index.html"))),
        )
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(server.clone(), guard))
        .with_state(server)
}
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let data = PathBuf::from(std::env::var("CHORES_DATA_DIR").unwrap_or("data".into()));
    std::fs::create_dir_all(&data)?;
    let db = initialize(&data.join("chores.sqlite3")).map_err(|e| e.1)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            data.join("chores.sqlite3"),
            std::fs::Permissions::from_mode(0o600),
        )?;
    }
    if std::env::args().nth(1).as_deref() == Some("--backup") {
        let path = std::env::args()
            .nth(2)
            .ok_or("Supply a destination for --backup")?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        db.backup("main", path, None)?;
        return Ok(());
    }
    let origin = std::env::var("CHORES_ORIGIN").unwrap_or("http://localhost:3101".into());
    let origin = origin.trim_end_matches('/').to_string();
    if !(origin.starts_with("http://") || origin.starts_with("https://")) {
        return Err("CHORES_ORIGIN must be an HTTP(S) origin".into());
    }
    let server = Server {
        db: Arc::new(Mutex::new(db)),
        origin: Arc::new(origin),
        limits: Arc::new(Mutex::new(HashMap::new())),
    };
    let bind = std::env::var("CHORES_BIND").unwrap_or("127.0.0.1:3002".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("Chores listening on {bind}");
    axum::serve(
        listener,
        app(
            server,
            PathBuf::from(std::env::var("CHORES_STATIC_DIR").unwrap_or("dist".into())),
        ),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
