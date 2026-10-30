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
        csp: Arc::new(content_security_policy(None)),
        oidc: None,
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
        db.execute(
            "INSERT INTO sessions(hash,person,expires) VALUES('session','a',9999999999)",
            [],
        )
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

#[test]
fn paired_calendar_operations_are_idempotent_and_revision_guarded() {
    let s = server();
    let mut db = s.conn().unwrap();
    db.execute(
        "INSERT INTO household VALUES(1,?1)",
        [serde_json::to_string(&Household::new(settings())).unwrap()],
    )
    .unwrap();
    let today = domain::date("2026-10-01").unwrap();
    let mut m = Mutation {
        id: "calendar-idempotent-swap".into(),
        revision: 0,
        operation: Operation::Swap {
            first: "2026-10-01".into(),
            second: "2026-10-02".into(),
        },
    };
    mutate_db(&mut db, "a", m.clone(), today).unwrap();
    mutate_db(&mut db, "a", m.clone(), today).unwrap();
    let h = read(&db).unwrap();
    assert_eq!(h.bedtime_events.len(), 1);
    let id = h.schedule().unwrap().swaps[0].swap.id.clone();
    m = Mutation {
        id: "calendar-idempotent-undo".into(),
        revision: 0,
        operation: Operation::UndoSwap { id },
    };
    assert!(mutate_db(&mut db, "b", m.clone(), today).is_err());
    assert_eq!(read(&db).unwrap().bedtime_events.len(), 1);
    m.revision = 1;
    mutate_db(&mut db, "b", m.clone(), today).unwrap();
    mutate_db(&mut db, "b", m, today).unwrap();
    assert!(read(&db).unwrap().schedule().unwrap().overrides.is_empty());
    let m = Mutation {
        id: "calendar-idempotent-restart".into(),
        revision: 2,
        operation: Operation::RestartBedtime {
            date: "2026-10-03".into(),
            first_person: "b".into(),
        },
    };
    mutate_db(&mut db, "a", m.clone(), today).unwrap();
    mutate_db(&mut db, "a", m, today).unwrap();
    let h = read(&db).unwrap();
    assert_eq!(h.revision, 3);
    assert_eq!(h.bedtime_events.len(), 3);
    assert_eq!(h.assignee("2026-10-01").unwrap(), "a");
    assert_eq!(h.assignee("2026-10-03").unwrap(), "b");
}

#[test]
fn database_upgrade_preserves_accounts_credits_and_calendar_once() {
    let path =
        std::env::temp_dir().join(format!("chores-migration-{}.sqlite3", uuid::Uuid::new_v4()));
    let mut legacy = serde_json::to_value(Household::new(settings())).unwrap();
    let object = legacy.as_object_mut().unwrap();
    object.remove("bedtimeEvents");
    object.insert(
        "overrides".into(),
        json!({"2026-10-01":"b","2026-10-02":"a"}),
    );
    object.insert("swaps".into(),json!([{"id":"old-swap","first":"2026-10-01","second":"2026-10-02","firstPerson":"a","secondPerson":"b","actor":"a","createdAt":"now"}]));
    object["chores"].as_array_mut().unwrap().push(json!({"id":"bedtime","name":"Put kids to sleep","icon":"bedtime","category":"Bedtime","points":5,"intervalDays":null,"archived":false}));
    object["completions"].as_array_mut().unwrap().push(json!({"id":"old-credit","choreId":"bedtime","name":"Put kids to sleep","icon":"bedtime","person":"b","points":5,"date":"2026-10-01","note":"","createdAt":"now","undone":false}));
    {
        let db = initialize(&path).unwrap();
        db.execute("INSERT INTO household VALUES(1,?1)", [legacy.to_string()])
            .unwrap();
        db.execute(
            "INSERT INTO users VALUES('a','fixture@example.test','saved-hash','saved-recovery')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions(hash,person,expires) VALUES('saved-session','a',9999999999)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO operations VALUES('old-operation','a','saved-payload')",
            [],
        )
        .unwrap();
        db.pragma_update(None, "user_version", 1).unwrap();
    }
    // A malformed calendar aborts the migration without advancing the schema or changing state.
    let mut invalid = legacy.clone();
    invalid["overrides"]["2026-10-01"] = json!("a");
    {
        let db = Connection::open(&path).unwrap();
        db.execute("UPDATE household SET state=?1", [invalid.to_string()])
            .unwrap();
    }
    assert!(initialize(&path).is_err());
    {
        let db = Connection::open(&path).unwrap();
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT state FROM household", [], |r| r.get::<_, String>(0))
                .unwrap(),
            invalid.to_string()
        );
        db.execute("UPDATE household SET state=?1", [legacy.to_string()])
            .unwrap();
    }
    for _ in 0..2 {
        let db = initialize(&path).unwrap();
        let h = read(&db).unwrap();
        assert_eq!(h.revision, 1);
        assert!(!h.chores.iter().any(|c| c.id == "bedtime"));
        assert_eq!(h.completions[0].points, 5);
        assert_eq!(h.assignee("2026-10-01").unwrap(), "b");
        assert_eq!(h.schedule().unwrap().swaps.len(), 1);
        assert_eq!(
            db.query_row("SELECT password FROM users WHERE id='a'", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "saved-hash"
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM operations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            3
        );
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn csp_allows_only_a_configured_feedback_hub() {
    let plain = content_security_policy(None);
    assert!(plain.contains("script-src 'self';"));
    assert!(plain.contains("connect-src 'self';"));
    let hub = content_security_policy(Some("https://feedback.example.org"));
    assert!(hub.contains(&format!(
        "script-src 'self' https://feedback.example.org {FEEDBACK_EARLY_PATCH_HASH};"
    )));
    assert!(hub.contains("connect-src 'self' https://feedback.example.org;"));
}

fn two_accounts(server: &Server) {
    let db = server.conn().unwrap();
    db.execute(
        "INSERT INTO users VALUES('a','alex@example.test','hash','r1'),('b','sam@example.test','hash','r2')",
        [],
    )
    .unwrap();
}
fn provider_config() -> oidc::OidcConfig {
    oidc::OidcConfig {
        issuer: "https://id.example.org".into(),
        client_id: "chores".into(),
        client_secret: "s".into(),
        redirect_url: "http://localhost/auth/callback".into(),
        allowed_groups: vec!["family".into()],
        user_map: oidc::parse_user_map("alexuser:a,samuser:SAM@example.test").unwrap(),
    }
}
fn identity(sub: &str, username: &str) -> oidc::Identity {
    oidc::Identity {
        issuer: "https://id.example.org".into(),
        sub: sub.into(),
        username: Some(username.into()),
    }
}
#[test]
fn provider_identities_link_once_through_the_user_map() {
    let server = server();
    two_accounts(&server);
    let config = provider_config();
    let mut db = server.conn().unwrap();
    assert_eq!(
        link_identity(&mut db, &config, &identity("u1", "AlexUser")).unwrap(),
        "a"
    );
    assert_eq!(
        link_identity(&mut db, &config, &identity("u2", "samuser")).unwrap(),
        "b"
    );
    // Once linked, the identity is found by subject even if the username changes.
    assert_eq!(
        link_identity(&mut db, &config, &identity("u1", "renamed")).unwrap(),
        "a"
    );
    // Unmapped users and a second identity for a linked account are refused.
    assert_eq!(
        link_identity(&mut db, &config, &identity("u3", "kid"))
            .unwrap_err()
            .0,
        "not-linked"
    );
    assert_eq!(
        link_identity(&mut db, &config, &identity("u4", "alexuser"))
            .unwrap_err()
            .0,
        "not-linked"
    );
}
#[tokio::test]
async fn provider_sessions_last_a_year_and_slide() {
    let server = server();
    two_accounts(&server);
    let headers = create_session(&server, &server.conn().unwrap(), "b", true).unwrap();
    let set = headers[header::SET_COOKIE].to_str().unwrap().to_string();
    assert!(set.contains("Max-Age=31536000"), "{set}");
    let raw = set
        .strip_prefix("chores_session=")
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let cookie = format!("chores_session={raw}");
    // A fresh session is not renewed on every request.
    let (status, headers, body) =
        request(server.clone(), "/api/auth/status", None, Some(&cookie), "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["person"], "b");
    assert_eq!(body["oidc"], false);
    assert!(headers.get(header::SET_COOKIE).is_none());
    // An older one is extended to a full year again, with a renewed cookie.
    let soon = Utc::now().timestamp() + 30 * 86400;
    server
        .conn()
        .unwrap()
        .execute("UPDATE sessions SET expires=?1", [soon])
        .unwrap();
    let (_, headers, _) =
        request(server.clone(), "/api/auth/status", None, Some(&cookie), "").await;
    assert!(
        headers[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=31536000")
    );
    let expires: i64 = server
        .conn()
        .unwrap()
        .query_row("SELECT expires FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert!(expires > Utc::now().timestamp() + 364 * 86400);
    // Password sessions keep their fixed 90 days.
    server
        .conn()
        .unwrap()
        .execute("UPDATE sessions SET expires=?1, sliding=0", [soon])
        .unwrap();
    let (_, headers, body) =
        request(server.clone(), "/api/auth/status", None, Some(&cookie), "").await;
    assert_eq!(body["person"], "b");
    assert!(headers.get(header::SET_COOKIE).is_none());
}
#[tokio::test]
async fn provider_routes_are_absent_without_configuration() {
    let (status, _, _) = request(server(), "/auth/login", None, None, "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = request(server(), "/auth/callback?code=x&state=y", None, None, "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn stored_argon2id_hashes_still_verify() {
    // PHC string in the format stored by earlier releases (argon2id v19).
    let stored = "$argon2id$v=19$m=16384,t=2,p=1$c2FsdHNhbHRzYWx0MTZi$paXuM2e2ddmdHrFD1sZ6XaWvIPRbO5rou8z7hWzHeI4";
    assert!(
        super::verify("correct horse battery".into(), stored.into())
            .await
            .unwrap()
    );
    assert!(
        !super::verify("wrong horse battery".into(), stored.into())
            .await
            .unwrap()
    );
    let fresh = super::hash("correct horse battery".into()).await.unwrap();
    assert!(fresh.starts_with("$argon2id$v=19$"));
    assert!(
        super::verify("correct horse battery".into(), fresh)
            .await
            .unwrap()
    );
}
