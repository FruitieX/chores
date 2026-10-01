use super::*;
use axum::body::{Body, to_bytes};
use tower::ServiceExt;
fn server() -> Server {
    Server {
        db: Arc::new(Mutex::new(
            initialize(std::path::Path::new(":memory:")).unwrap(),
        )),
        origin: Arc::new("http://localhost".into()),
        limits: Arc::new(Mutex::new(HashMap::new())),
    }
}
fn settings() -> Settings {
    Settings {
        household_name: "Home".into(),
        names: ["Alex".into(), "Sam".into()],
        balance_days: 30,
        bedtime_start: "2026-10-01".into(),
        bedtime_first: "a".into(),
    }
}
async fn request(
    server: Server,
    path: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    origin: &str,
) -> (StatusCode, HeaderMap, Value) {
    let mut builder = axum::http::Request::builder()
        .uri(path)
        .header(header::ORIGIN, origin);
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    let request = if let Some(body) = body {
        builder
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };
    let response = app(server, PathBuf::from("/nonexistent"))
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[test]
fn idempotency_and_stale_revision_are_atomic() {
    let server = server();
    let mut db = server.conn().unwrap();
    db.execute(
        "INSERT INTO household VALUES(1,?1)",
        [serde_json::to_string(&Household::new(settings())).unwrap()],
    )
    .unwrap();
    let mutation = || Mutation {
        id: "test-operation-0000001".into(),
        revision: 0,
        operation: Operation::Complete {
            chore_id: "washer".into(),
            expected_points: 2,
            date: "2026-10-01".into(),
            note: "".into(),
        },
    };
    let today = domain::date("2026-10-01").unwrap();
    mutate_db(&mut db, "a", mutation(), today).unwrap();
    mutate_db(&mut db, "a", mutation(), today).unwrap();
    assert_eq!(read(&db).unwrap().completions.len(), 1);
    assert!(mutate_db(&mut db, "b", mutation(), today).is_err());
    let mut stale = mutation();
    stale.id = "test-operation-0000002".into();
    stale.operation = Operation::Swap {
        first: "2026-10-01".into(),
        second: "2026-10-02".into(),
    };
    assert!(mutate_db(&mut db, "a", stale, today).is_err());
    assert_eq!(read(&db).unwrap().revision, 1);
}
#[test]
fn independent_completions_can_share_a_revision() {
    let server = server();
    let mut db = server.conn().unwrap();
    db.execute(
        "INSERT INTO household VALUES(1,?1)",
        [serde_json::to_string(&Household::new(settings())).unwrap()],
    )
    .unwrap();
    for person in ["a", "b"] {
        mutate_db(
            &mut db,
            person,
            Mutation {
                id: format!("test-independent-person-{person}"),
                revision: 0,
                operation: Operation::Complete {
                    chore_id: "washer".into(),
                    expected_points: 2,
                    date: "2026-10-01".into(),
                    note: "".into(),
                },
            },
            domain::date("2026-10-01").unwrap(),
        )
        .unwrap();
    }
    assert_eq!(read(&db).unwrap().completions.len(), 2);
}
#[tokio::test]
async fn setup_once_separate_accounts_origin_and_recovery() {
    let s = server();
    let body = json!({"settings":settings(),"emails":["alex@example.test","sam@example.test"],"passwords":["first-password-long","second-password-long"]});
    assert_eq!(
        request(
            s.clone(),
            "/api/auth/setup",
            Some(body.clone()),
            None,
            "http://evil.test"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, headers, data) = request(
        s.clone(),
        "/api/auth/setup",
        Some(body.clone()),
        None,
        "http://localhost",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let cookie = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert_eq!(
        request(
            s.clone(),
            "/api/auth/setup",
            Some(body),
            None,
            "http://localhost"
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(s.clone(), "/api/state", None, None, "http://localhost")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            s.clone(),
            "/api/state",
            None,
            Some(&cookie),
            "http://localhost"
        )
        .await
        .2["person"],
        "a"
    );
    let (status, _, partner) = request(
        s.clone(),
        "/api/auth/login",
        Some(json!({"email":"sam@example.test","password":"second-password-long"})),
        None,
        "http://localhost",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(partner["person"], "b");
    let old_code = data["recoveryCodes"][0].as_str().unwrap();
    let (status, _, recovered) = request(
        s.clone(),
        "/api/auth/recover",
        Some(json!({"email":"alex@example.test","code":old_code,"password":"new-password-long"})),
        None,
        "http://localhost",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(recovered["recoveryCodes"]["code"], old_code);
    assert_eq!(
        request(
            s.clone(),
            "/api/state",
            None,
            Some(&cookie),
            "http://localhost"
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            s.clone(),
            "/api/auth/recover",
            Some(
                json!({"email":"alex@example.test","code":old_code,"password":"new-password-long"})
            ),
            None,
            "http://localhost"
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn database_restart_preserves_state_and_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("chores.db");
    {
        let db = initialize(&path).unwrap();
        db.execute(
            "INSERT INTO household VALUES(1,?1)",
            [serde_json::to_string(&Household::new(settings())).unwrap()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO users VALUES('a','a@example.test','hash','recovery')",
            [],
        )
        .unwrap();
        db.execute("INSERT INTO sessions VALUES('session','a',9999999999)", [])
            .unwrap();
    }
    let db = initialize(&path).unwrap();
    assert_eq!(read(&db).unwrap().settings.names[0], "Alex");
    assert_eq!(
        db.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
}

#[test]
fn password_rotation_blocks_a_login_verified_against_old_credentials() {
    let server = server();
    server
        .conn()
        .unwrap()
        .execute(
            "INSERT INTO users VALUES('a','alex@example.test','new-password-hash','recovery')",
            [],
        )
        .unwrap();
    assert!(sign_in(&server, "a", Value::Null, "old-password-hash").is_err());
    assert_eq!(
        server
            .conn()
            .unwrap()
            .query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert!(sign_in(&server, "a", Value::Null, "new-password-hash").is_ok());
}
