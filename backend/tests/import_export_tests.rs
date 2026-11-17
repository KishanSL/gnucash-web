//! GnuCash SQLite/XML import and SQLite export (IMP-*, EXP-*)

mod common;
use common::*;

#[tokio::test]
async fn test_import_gnucash_sqlite() {
    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    // 1. Create a temporary in-file GnuCash SQLite DB
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();

    conn.execute_batch(
        r#"
        CREATE TABLE commodities (
            guid VARCHAR(32) PRIMARY KEY,
            namespace VARCHAR(2048),
            mnemonic VARCHAR(2048),
            fullname VARCHAR(2048),
            fraction INTEGER
        );
        INSERT INTO commodities VALUES ('c1111111111111111111111111111111', 'CURRENCY', 'INR', 'Indian Rupee', 100);

        CREATE TABLE accounts (
            guid VARCHAR(32) PRIMARY KEY,
            name VARCHAR(2048) NOT NULL,
            account_type VARCHAR(2048) NOT NULL,
            commodity_guid VARCHAR(32),
            commodity_scu INTEGER,
            non_std_scu INTEGER,
            parent_guid VARCHAR(32),
            code VARCHAR(2048),
            description VARCHAR(2048),
            hidden INTEGER,
            placeholder INTEGER
        );
        INSERT INTO accounts VALUES ('00000000000000000000000000000001', 'Root Account', 'ROOT', NULL, 100, 0, NULL, '', '', 0, 1);
        INSERT INTO accounts VALUES ('00000000000000000000000000000002', 'Assets', 'ASSET', 'c1111111111111111111111111111111', 100, 0, '00000000000000000000000000000001', '1000', 'Top Assets', 0, 1);
        INSERT INTO accounts VALUES ('00000000000000000000000000000003', 'HDFC Bank Account', 'BANK', 'c1111111111111111111111111111111', 100, 0, '00000000000000000000000000000002', '1110', 'Primary Checking', 0, 0);
        INSERT INTO accounts VALUES ('00000000000000000000000000000004', 'Expenses', 'EXPENSE', 'c1111111111111111111111111111111', 100, 0, '00000000000000000000000000000001', '5000', 'Operating Expenses', 0, 1);
        INSERT INTO accounts VALUES ('00000000000000000000000000000005', 'Food & Groceries', 'EXPENSE', 'c1111111111111111111111111111111', 100, 0, '00000000000000000000000000000004', '5100', 'Daily food groceries', 0, 0);

        CREATE TABLE transactions (
            guid VARCHAR(32) PRIMARY KEY,
            currency_guid VARCHAR(32),
            num VARCHAR(2048),
            post_date VARCHAR(14) NOT NULL,
            enter_date VARCHAR(14),
            description VARCHAR(2048)
        );
        INSERT INTO transactions VALUES ('t0000000000000000000000000000001', 'c1111111111111111111111111111111', 'TX-01', '20261025120000', '20261025120000', 'Supermarket Mart');

        CREATE TABLE splits (
            guid VARCHAR(32) PRIMARY KEY,
            tx_guid VARCHAR(32) NOT NULL,
            account_guid VARCHAR(32) NOT NULL,
            memo VARCHAR(2048),
            action VARCHAR(2048),
            reconcile_state VARCHAR(1),
            reconcile_date VARCHAR(14),
            value_num BIGINT NOT NULL,
            value_denom BIGINT NOT NULL,
            quantity_num BIGINT,
            quantity_denom BIGINT,
            lot_guid VARCHAR(32)
        );
        -- Debit Food (₹1,500.00), Credit Bank (-₹1,500.00)
        INSERT INTO splits VALUES ('s0000000000000000000000000000001', 't0000000000000000000000000000001', '00000000000000000000000000000005', 'Vegetables and dairy', '', 'n', NULL, 150000, 100, 150000, 100, NULL);
        INSERT INTO splits VALUES ('s0000000000000000000000000000002', 't0000000000000000000000000000001', '00000000000000000000000000000003', 'UPI payment', '', 'y', '20261025120000', -150000, 100, -150000, 100, NULL);
        "#,
    ).unwrap();

    let file_bytes = std::fs::read(temp_db.path()).unwrap();

    // 2. Build multipart body
    let boundary = "---------------------------974767299852498929531610575";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"my_finances.gnucash\"\r\n");
    body.extend_from_slice(b"Content-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&file_bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/gnucash-sqlite?wipe=false")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let res_body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&res_body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["accounts_imported"], 4);
    assert_eq!(json["transactions_imported"], 1);
    assert_eq!(json["splits_imported"], 2);
}

#[tokio::test]
async fn test_import_gnucash_compressed_xml() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let pool = get_test_pool().await;
    let app = create_app(pool.clone());

    let xml_data = r#"<?xml version="1.0" encoding="utf-8" ?>
<gnc-v2 xmlns:gnc="http://www.gnucash.org/XML/gnc" xmlns:act="http://www.gnucash.org/XML/act" xmlns:trn="http://www.gnucash.org/XML/trn" xmlns:split="http://www.gnucash.org/XML/split" xmlns:cmdty="http://www.gnucash.org/XML/cmdty" xmlns:ts="http://www.gnucash.org/XML/ts" xmlns:slot="http://www.gnucash.org/XML/slot">
<gnc:book version="2.0.0">
  <gnc:account version="2.0.0">
    <act:name>Root Account</act:name>
    <act:id type="guid">10000000000000000000000000000000</act:id>
    <act:type>ROOT</act:type>
  </gnc:account>
  <gnc:account version="2.0.0">
    <act:name>Assets</act:name>
    <act:id type="guid">20000000000000000000000000000000</act:id>
    <act:type>ASSET</act:type>
    <act:parent type="guid">10000000000000000000000000000000</act:parent>
  </gnc:account>
  <gnc:account version="2.0.0">
    <act:name>Checking</act:name>
    <act:id type="guid">30000000000000000000000000000000</act:id>
    <act:type>BANK</act:type>
    <act:parent type="guid">20000000000000000000000000000000</act:parent>
  </gnc:account>
  <gnc:account version="2.0.0">
    <act:name>Salary</act:name>
    <act:id type="guid">40000000000000000000000000000000</act:id>
    <act:type>INCOME</act:type>
    <act:parent type="guid">10000000000000000000000000000000</act:parent>
  </gnc:account>
  <gnc:transaction version="2.0.0">
    <trn:id type="guid">50000000000000000000000000000000</trn:id>
    <trn:date-posted><ts:date>2026-05-01 12:00:00 +0000</ts:date></trn:date-posted>
    <trn:description>May Paycheck</trn:description>
    <trn:splits>
      <trn:split>
        <split:id type="guid">60000000000000000000000000000000</split:id>
        <split:account type="guid">30000000000000000000000000000000</split:account>
        <split:value>7500000/100</split:value>
        <split:reconciled-state>n</split:reconciled-state>
      </trn:split>
      <trn:split>
        <split:id type="guid">70000000000000000000000000000000</split:id>
        <split:account type="guid">40000000000000000000000000000000</split:account>
        <split:value>-7500000/100</split:value>
        <split:reconciled-state>n</split:reconciled-state>
      </trn:split>
    </trn:splits>
  </gnc:transaction>
</gnc:book>
</gnc-v2>"#;

    // Compress with Gzip
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(xml_data.as_bytes()).unwrap();
    let gzip_bytes = encoder.finish().unwrap();

    let boundary = "---------------------------12345678901234567890";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"sample.gnucash\"\r\n");
    body.extend_from_slice(b"Content-Type: application/gzip\r\n\r\n");
    body.extend_from_slice(&gzip_bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/gnucash-sqlite?wipe=false")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let res_body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&res_body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["accounts_imported"], 3);
    assert_eq!(json["transactions_imported"], 1);
    assert_eq!(json["splits_imported"], 2);
}

#[tokio::test]
async fn test_import_from_path_is_disabled() {
    let pool = get_test_pool().await;
    let app = create_app(pool);

    let payload = json!({
        "path": "/etc/passwd",
        "wipe": false,
        "default_currency": "INR"
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/from-path")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_transaction_notes_roundtrip_import_export() {
    // Issue #11: Transaction notes preserved through export and import
    let pool = get_test_pool().await;

    let user_id = uuid::Uuid::new_v4();
    let username = format!("notestest_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')"
    )
    .bind(user_id)
    .bind(&username)
    .bind(&email)
    .execute(&pool)
    .await
    .unwrap();

    let user_token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let acc1_id = uuid::Uuid::new_v4();
    let acc2_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, currency, placeholder, user_id) VALUES ($1, 'Checking', 'bank', 'INR', false, $2), ($3, 'Groceries', 'expense', 'INR', false, $2)")
        .bind(acc1_id)
        .bind(user_id)
        .bind(acc2_id)
        .execute(&pool)
        .await
        .unwrap();

    let app = create_app(pool.clone());
    let create_payload = json!({
        "post_date": "2026-11-05",
        "description": "Weekly Grocery Run",
        "notes": "Paid with GPay; receipt saved in drawer",
        "splits": [
            { "account_id": acc2_id, "amount": "1500.00" },
            { "account_id": acc1_id, "amount": "-1500.00" }
        ]
    });
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/transactions")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", "application/json")
            .body(Body::from(create_payload.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Export SQLite database
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/export/gnucash-sqlite")
            .header("authorization", format!("Bearer {}", user_token))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let sqlite_bytes = resp.into_body().collect().await.unwrap().to_bytes();

    // Verify SQLite slots table has the notes
    let temp_db = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_db.path(), &sqlite_bytes).unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();
    let slot_val: String = conn.query_row(
        "SELECT string_val FROM slots WHERE name = 'notes'",
        [],
        |row| row.get(0)
    ).unwrap();
    assert_eq!(slot_val, "Paid with GPay; receipt saved in drawer");

    // Import this SQLite file into a new fresh user
    let user2_id = uuid::Uuid::new_v4();
    let username2 = format!("notestest2_{}", &user2_id.to_string()[..8]);
    let email2 = format!("{}@example.com", username2);
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')"
    )
    .bind(user2_id)
    .bind(&username2)
    .bind(&email2)
    .execute(&pool)
    .await
    .unwrap();
    let user2_token = gnucash_backend::auth::create_jwt(user2_id, &username2, &email2, 0).unwrap();

    let boundary = "---------------------------974767299852498929531610575";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"test_notes.gnucash\"\r\n");
    body.extend_from_slice(b"Content-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&sqlite_bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=true")
            .header("authorization", format!("Bearer {}", user2_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify imported transaction has notes preserved in Postgres
    let imported_notes: Option<String> = sqlx::query_scalar(
        "SELECT notes FROM transactions WHERE user_id = $1"
    )
    .bind(user2_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(imported_notes.as_deref(), Some("Paid with GPay; receipt saved in drawer"));
}

#[tokio::test]
async fn test_reimport_without_wipe_replaces_stale_splits() {
    // Issue #5: Re-import without wipe removes stale splits and keeps balance intact
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("reimport_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let user_token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let temp_db1 = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db1.path()).unwrap();
    conn.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expenses', 'EXPENSE', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO transactions VALUES ('t1', 'c1', '1', '20261101105900', '20261101105900', 'Grocery');
        INSERT INTO splits VALUES ('s1', 't1', 'a1', '', '', 'n', NULL, -10000, 100, -10000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 't1', 'a2', '', '', 'n', NULL, 10000, 100, 10000, 100, NULL);
    "#).unwrap();

    let bytes1 = std::fs::read(temp_db1.path()).unwrap();
    let boundary = "---------------------------11223344556677889900";
    let mut body1 = Vec::new();
    body1.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body1.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"book1.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body1.extend_from_slice(&bytes1);
    body1.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=false")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body1))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Modify SQLite DB: change amount from 100 to 120 and use new split guid s3
    let temp_db2 = tempfile::NamedTempFile::new().unwrap();
    let conn2 = rusqlite::Connection::open(temp_db2.path()).unwrap();
    conn2.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expenses', 'EXPENSE', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO transactions VALUES ('t1', 'c1', '1', '20261101105900', '20261101105900', 'Grocery Updated');
        INSERT INTO splits VALUES ('s1', 't1', 'a1', '', '', 'n', NULL, -12000, 100, -12000, 100, NULL);
        INSERT INTO splits VALUES ('s3', 't1', 'a2', '', '', 'n', NULL, 12000, 100, 12000, 100, NULL);
    "#).unwrap();

    let bytes2 = std::fs::read(temp_db2.path()).unwrap();
    let mut body2 = Vec::new();
    body2.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body2.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"book2.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body2.extend_from_slice(&bytes2);
    body2.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=false")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body2))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify there are exactly 2 splits in total, with amounts 120 and -120
    let split_amounts: Vec<rust_decimal::Decimal> = sqlx::query_scalar(
        "SELECT amount FROM splits WHERE transaction_id IN (SELECT id FROM transactions WHERE user_id = $1) ORDER BY amount ASC"
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(split_amounts.len(), 2, "Stale split s2 must have been deleted, leaving exactly 2 splits");
    assert_eq!(split_amounts[0], rust_decimal_macros::dec!(-120.00));
    assert_eq!(split_amounts[1], rust_decimal_macros::dec!(120.00));
}

#[tokio::test]
async fn test_importer_ignores_scheduled_transaction_templates() {
    // Issue #7: Importer ignores scheduled-transaction templates
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("sxtest_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let user_token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();
    conn.execute_batch(r#"
        CREATE TABLE books (guid TEXT PRIMARY KEY, root_account_guid TEXT, root_template_guid TEXT);
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO books VALUES ('b1', 'real_root', 'template_root');
        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('real_root', 'Root Account', 'ROOT', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a1', 'Checking', 'BANK', 'c1', 'real_root', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expenses', 'EXPENSE', 'c1', 'real_root', NULL, NULL, 0, 0);

        -- Template tree
        INSERT INTO accounts VALUES ('template_root', 'Template Root', 'ROOT', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('sx_child_guid_1234567890abcdef', 'SX Account', 'BANK', 'c1', 'template_root', NULL, NULL, 0, 0);

        -- Real transaction
        INSERT INTO transactions VALUES ('t_real', 'c1', '', '20261101105900', '20261101105900', 'Real Transaction');
        INSERT INTO splits VALUES ('s1', 't_real', 'a1', '', '', 'n', NULL, -5000, 100, -5000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 't_real', 'a2', '', '', 'n', NULL, 5000, 100, 5000, 100, NULL);

        -- Template transaction
        INSERT INTO transactions VALUES ('t_template', 'c1', '', '20261101105900', '20261101105900', 'Template Transaction');
        INSERT INTO splits VALUES ('s_tmp1', 't_template', 'sx_child_guid_1234567890abcdef', '', '', 'n', NULL, 0, 100, 0, 100, NULL);
        INSERT INTO splits VALUES ('s_tmp2', 't_template', 'a2', '', '', 'n', NULL, 0, 100, 0, 100, NULL);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------44556677889900112233";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"sx.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=true")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify template account was NOT imported
    let sx_accts: Vec<String> = sqlx::query_scalar("SELECT name FROM accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!sx_accts.contains(&"SX Account".to_string()), "Template accounts must be excluded");
    assert!(!sx_accts.contains(&"Template Root".to_string()), "Template root must be excluded");

    // Verify only the real transaction exists
    let tx_descs: Vec<String> = sqlx::query_scalar("SELECT description FROM transactions WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(tx_descs, vec!["Real Transaction".to_string()]);
}

#[tokio::test]
async fn test_stock_foreign_commodity_import_and_export() {
    // Issue #9: Stock / ISIN mnemonic accounts (>10 chars) import and export cleanly
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("stock_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let user_token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();
    conn.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c_inr', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO commodities VALUES ('c_fund', 'FUND', 'INF209K01VA3', 'Aditya Birla Mutual Fund', 'INF209K01VA3', 1000, 1, 'financequote');

        INSERT INTO accounts VALUES ('a_bank', 'Bank Account', 'BANK', 'c_inr', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a_stock', 'Mutual Fund Investment', 'STOCK', 'c_fund', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('t_buy', 'c_inr', '', '20261101105900', '20261101105900', 'SIP Purchase');
        INSERT INTO splits VALUES ('s1', 't_buy', 'a_bank', '', '', 'n', NULL, -150000, 100, -150000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 't_buy', 'a_stock', '', '', 'n', NULL, 150000, 100, 10000, 1000, NULL);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------99887766554433221100";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"stock.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=true")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "Importing account with 12-char ISIN must succeed without VARCHAR truncation");

    let stock_currency: String = sqlx::query_scalar("SELECT currency FROM accounts WHERE name = 'Mutual Fund Investment' AND user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stock_currency, "INF209K01VA3");

    // Verify account balance reflects investment amount (1500), not units (Issue #21 scope decision)
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/accounts")
            .header("authorization", format!("Bearer {}", user_token))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let accs: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    let stock_acc = accs.iter().find(|a| a["name"] == "Mutual Fund Investment").unwrap();
    let bal: rust_decimal::Decimal = stock_acc["balance"].as_str().unwrap().parse().unwrap();
    assert_eq!(bal, rust_decimal::Decimal::from(1500), "Stock account balance must show investment amount, not units (Issue #21)");

    // Export again and verify FUND namespace and faithful quantities
    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/api/export/gnucash-sqlite")
            .header("authorization", format!("Bearer {}", user_token))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let exported_bytes = resp.into_body().collect().await.unwrap().to_bytes();

    let temp_exp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_exp.path(), &exported_bytes).unwrap();
    let exp_conn = rusqlite::Connection::open(temp_exp.path()).unwrap();
    let (namespace, fraction): (String, i64) = exp_conn.query_row(
        "SELECT namespace, fraction FROM commodities WHERE mnemonic = 'INF209K01VA3'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?))
    ).unwrap();
    assert_eq!(namespace, "FUND", "Exported commodity with >3 chars must have namespace FUND");
    assert_eq!(fraction, 1000, "Exported commodity fraction must be preserved");

    let (val_num, val_denom, qty_num, qty_denom): (i64, i64, i64, i64) = exp_conn.query_row(
        "SELECT s.value_num, s.value_denom, s.quantity_num, s.quantity_denom
         FROM splits s
         JOIN accounts a ON s.account_guid = a.guid
         WHERE a.name = 'Mutual Fund Investment'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    ).unwrap();
    assert_eq!(val_num, 150000);
    assert_eq!(val_denom, 100);
    assert_eq!(qty_num, 10000, "Split quantity_num must be preserved on export");
    assert_eq!(qty_denom, 1000, "Split quantity_denom must be preserved on export");
}

#[tokio::test]
async fn test_issue_19_reject_unbalanced_import() {
    // Issue #19: Importer must strictly reject transactions unbalanced by even 0.01
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("unbal_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let user_token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();
    conn.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c_inr', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'BANK', 'c_inr', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expenses', 'EXPENSE', 'c_inr', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('t1', 'c_inr', '', '20261101105900', '20261101105900', 'Unbalanced tx');
        -- Off by 0.01: -50.00 and +50.01
        INSERT INTO splits VALUES ('s1', 't1', 'a1', '', '', 'n', NULL, -5000, 100, -5000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 't1', 'a2', '', '', 'n', NULL, 5001, 100, 5001, 100, NULL);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------11223344556677889900";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"unbalanced.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=true")
            .header("authorization", format!("Bearer {}", user_token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body))
            .unwrap()
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Importing transaction off by 0.01 must return 400 Bad Request");
    let resp_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let resp_json: serde_json::Value = serde_json::from_slice(&resp_bytes).unwrap();
    assert!(resp_json["error"].as_str().unwrap().contains("Double-entry violation"));
}

#[tokio::test]
async fn test_export_versions_table_and_neutral_time() {
    // EXP-2: The versions table contains:
    // Gnucash = 5000017, Gnucash-Resave = 19920, transactions = 4, splits = 5, accounts = 1, commodities = 1, books = 1.
    // EXP-3: Post dates are written at GnuCash neutral time (YYYYMMDD105900).
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("expver_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    // Create accounts and a transaction for user
    let acc1 = uuid::Uuid::new_v4();
    let acc2 = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, user_id) VALUES ($1, 'Export Bank', 'bank', $3), ($2, 'Export Expense', 'expense', $3)")
        .bind(acc1)
        .bind(acc2)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    let tx1 = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, post_date, description, user_id) VALUES ($1, '2026-11-16', 'Export Date Test', $2)")
        .bind(tx1)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, -50.00, -50.00, 'n'), ($4, $2, $5, 50.00, 50.00, 'n')")
        .bind(uuid::Uuid::new_v4())
        .bind(tx1)
        .bind(acc1)
        .bind(uuid::Uuid::new_v4())
        .bind(acc2)
        .execute(&pool)
        .await
        .unwrap();

    // Export database
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/export/gnucash-sqlite")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();

    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), &bytes).unwrap();
    let conn = rusqlite::Connection::open(temp_file.path()).unwrap();

    // Verify versions table per EXP-2
    let mut stmt = conn.prepare("SELECT table_name, table_version FROM versions").unwrap();
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).unwrap();
    let versions: std::collections::HashMap<String, i64> = rows.filter_map(|r| r.ok()).collect();

    assert_eq!(versions.get("Gnucash"), Some(&5000017), "versions.Gnucash must be 5000017");
    assert_eq!(versions.get("Gnucash-Resave"), Some(&19920), "versions.Gnucash-Resave must be 19920");
    assert_eq!(versions.get("transactions"), Some(&4), "versions.transactions must be 4");
    assert_eq!(versions.get("splits"), Some(&5), "versions.splits must be 5");
    assert_eq!(versions.get("accounts"), Some(&1), "versions.accounts must be 1");
    assert_eq!(versions.get("commodities"), Some(&1), "versions.commodities must be 1");
    assert_eq!(versions.get("books"), Some(&1), "versions.books must be 1");

    // Verify post_date is written at GnuCash neutral time YYYYMMDD105900 per EXP-3
    let post_date: String = conn.query_row(
        "SELECT post_date FROM transactions LIMIT 1",
        [],
        |r| r.get(0)
    ).unwrap();
    assert!(post_date.ends_with("105900"), "post_date '{}' must end with neutral time 105900", post_date);
    assert_eq!(post_date, "20261116105900");
}

#[tokio::test]
async fn test_import_reconciled_state_normalization() {
    // IMP-8: GnuCash reconcile states other than n/c/y (e.g. f, v) are imported as n.
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("imp_rec_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let temp_db = tempfile::NamedTempFile::new().unwrap();
    let conn = rusqlite::Connection::open(temp_db.path()).unwrap();
    conn.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c_inr', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Bank Account', 'BANK', 'c_inr', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Expense Account', 'EXPENSE', 'c_inr', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('tx_rec', 'c_inr', '', '20261101105900', '20261101105900', 'Reconcile Normalization Tx');
        -- Split 1 has state 'f' (frozen), Split 2 has state 'v' (voided)
        INSERT INTO splits VALUES ('s1', 'tx_rec', 'a1', '', '', 'f', NULL, -2500, 100, -2500, 100, NULL);
        INSERT INTO splits VALUES ('s2', 'tx_rec', 'a2', '', '', 'v', NULL, 2500, 100, 2500, 100, NULL);
    "#).unwrap();

    let bytes = std::fs::read(temp_db.path()).unwrap();
    let boundary = "---------------------------recstateboundary12345";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"reconcile_states.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let app = create_app(pool.clone());
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/import/gnucash-sqlite?wipe=true")
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", format!("multipart/form-data; boundary={}", boundary))
            .body(Body::from(body))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify both splits in Postgres are normalized to 'n'
    let states: Vec<String> = sqlx::query_scalar(
        "SELECT reconciled_state FROM splits s JOIN transactions t ON s.transaction_id = t.id WHERE t.user_id = $1"
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(states.len(), 2);
    for state in states {
        assert_eq!(state, "n", "Non-standard reconcile state ('f' or 'v') must be normalized to 'n'");
    }
}

#[tokio::test]
async fn test_issue_35_import_field_length_and_error_identification() {
    // Issue #35: IMP-11, TXN-1, ACC-11, ACC-2, SEC-13
    // Validate field lengths before DB transaction and name offending items in error responses
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("imp11_{}", &user_id.to_string()[..8]);
    let email = format!("{}@example.com", username);
    sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, 'hash')")
        .bind(user_id)
        .bind(&username)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let upload_sqlite = |db_bytes: Vec<u8>| {
        let app = create_app(pool.clone());
        let tok = token.clone();
        async move {
            let boundary = "---------------------------boundaryimp11test";
            let mut body = Vec::new();
            body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
            body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"test.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n");
            body.extend_from_slice(&db_bytes);
            body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/import/gnucash-sqlite?wipe=true")
                    .header("authorization", format!("Bearer {}", tok))
                    .header("content-type", format!("multipart/form-data; boundary={}", boundary))
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    // 1. Transaction description > 255 chars (IMP-11, TXN-1)
    let temp_db1 = tempfile::NamedTempFile::new().unwrap();
    let conn1 = rusqlite::Connection::open(temp_db1.path()).unwrap();
    let long_desc = format!("LongTx_{}", "X".repeat(250)); // 257 characters
    conn1.execute_batch(&format!(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'USD', 'USD', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Checking', 'BANK', 'c1', 'a1', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a3', 'Groceries', 'EXPENSE', 'c1', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('tx_long', 'c1', '', '20261101100000', '20261101100000', '{long_desc}');
        INSERT INTO splits VALUES ('s1', 'tx_long', 'a2', '', '', 'n', NULL, -5000, 100, -5000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 'tx_long', 'a3', '', '', 'n', NULL, 5000, 100, 5000, 100, NULL);
    "#)).unwrap();

    let bytes1 = std::fs::read(temp_db1.path()).unwrap();
    let resp1 = upload_sqlite(bytes1).await;
    assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);
    let body_bytes1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let json1: Value = serde_json::from_slice(&body_bytes1).unwrap();
    let err_msg1 = json1["error"].as_str().unwrap();

    // Verify error identifies post date, start of description, split accounts, and limit (IMP-11)
    assert!(err_msg1.contains("2026-11-01"), "Error must contain post date: {}", err_msg1);
    assert!(err_msg1.contains("LongTx_"), "Error must contain start of description: {}", err_msg1);
    assert!(err_msg1.contains("Assets:Checking") || err_msg1.contains("Groceries"), "Error must contain split accounts: {}", err_msg1);
    assert!(err_msg1.contains("255"), "Error must name limit 255: {}", err_msg1);
    // Verify no raw DB error text leaked (SEC-13)
    assert!(!err_msg1.contains("error returned from database"));
    assert!(!err_msg1.contains("character varying"));

    // Verify nothing written to DB
    let tx_count1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tx_count1, 0, "No transactions should be written on validation error");

    // 2. Account code > 50 chars (IMP-11, ACC-11)
    let temp_db2 = tempfile::NamedTempFile::new().unwrap();
    let conn2 = rusqlite::Connection::open(temp_db2.path()).unwrap();
    let long_code = "C".repeat(55); // 55 characters
    conn2.execute_batch(&format!(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'USD', 'USD', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Savings', 'BANK', 'c1', 'a1', '{long_code}', NULL, 0, 0);
    "#)).unwrap();

    let bytes2 = std::fs::read(temp_db2.path()).unwrap();
    let resp2 = upload_sqlite(bytes2).await;
    assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);
    let body_bytes2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body_bytes2).unwrap();
    let err_msg2 = json2["error"].as_str().unwrap();

    // Verify error names full account name and limit 50 (IMP-11)
    assert!(err_msg2.contains("Assets:Savings"), "Error must name full account name: {}", err_msg2);
    assert!(err_msg2.contains("50"), "Error must name limit 50: {}", err_msg2);
    assert!(!err_msg2.contains("error returned from database"));
    assert!(!err_msg2.contains("character varying"));

    let acc_count2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(acc_count2, 0, "No accounts should be written on validation error");

    // 3. Unbalanced transaction names date, description, and accounts (ACC-2, IMP-11)
    let temp_db3 = tempfile::NamedTempFile::new().unwrap();
    let conn3 = rusqlite::Connection::open(temp_db3.path()).unwrap();
    conn3.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'USD', 'USD', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Cash', 'CASH', 'c1', 'a1', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a3', 'Coffee', 'EXPENSE', 'c1', NULL, NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('tx_unbalanced', 'c1', '', '20261102120000', '20261102120000', 'Unbalanced Coffee');
        INSERT INTO splits VALUES ('s1', 'tx_unbalanced', 'a2', '', '', 'n', NULL, -450, 100, -450, 100, NULL);
        INSERT INTO splits VALUES ('s2', 'tx_unbalanced', 'a3', '', '', 'n', NULL, 500, 100, 500, 100, NULL);
    "#).unwrap();

    let bytes3 = std::fs::read(temp_db3.path()).unwrap();
    let resp3 = upload_sqlite(bytes3).await;
    assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);
    let body_bytes3 = resp3.into_body().collect().await.unwrap().to_bytes();
    let json3: Value = serde_json::from_slice(&body_bytes3).unwrap();
    let err_msg3 = json3["error"].as_str().unwrap();

    assert!(err_msg3.contains("Double-entry violation"), "Must maintain compatibility with Double-entry violation: {}", err_msg3);
    assert!(err_msg3.contains("tx_unbalanced"), "Must identify transaction GUID: {}", err_msg3);
    assert!(err_msg3.contains("2026-11-02"), "Must identify post date: {}", err_msg3);
    assert!(err_msg3.contains("Unbalanced Coffee"), "Must identify description: {}", err_msg3);
    assert!(err_msg3.contains("Assets:Cash") || err_msg3.contains("Coffee"), "Must identify accounts: {}", err_msg3);
    assert!(err_msg3.contains("0.5"), "Must identify imbalance: {}", err_msg3);
    assert!(!err_msg3.contains("error returned from database"));
}

#[tokio::test]
async fn test_issue_23_xml_import_entities() {
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("user_xml_{}", user_id.simple());
    let email = format!("user_xml_{}@example.com", user_id.simple());

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, token_version) VALUES ($1, $2, $3, 'hash', 0)"
    )
    .bind(user_id)
    .bind(&username)
    .bind(&email)
    .execute(&pool)
    .await
    .unwrap();

    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let xml = r#"<?xml version="1.0" encoding="utf-8" ?>
<gnc-v2 xmlns:gnc="http://www.gnucash.org/XML/gnc"
        xmlns:act="http://www.gnucash.org/XML/act"
        xmlns:trn="http://www.gnucash.org/XML/trn"
        xmlns:split="http://www.gnucash.org/XML/split"
        xmlns:cmdty="http://www.gnucash.org/XML/cmdty"
        xmlns:ts="http://www.gnucash.org/XML/ts"
        xmlns:slot="http://www.gnucash.org/XML/slot">
  <gnc:book>
    <gnc:account>
      <act:name>Food &amp; Dining</act:name>
      <act:id type="guid">acc-1</act:id>
      <act:type>EXPENSE</act:type>
      <act:commodity>
        <cmdty:space>CURRENCY</cmdty:space>
        <cmdty:id>INR</cmdty:id>
      </act:commodity>
    </gnc:account>
    <gnc:account>
      <act:name>Checking Account</act:name>
      <act:id type="guid">acc-2</act:id>
      <act:type>BANK</act:type>
      <act:commodity>
        <cmdty:space>CURRENCY</cmdty:space>
        <cmdty:id>INR</cmdty:id>
      </act:commodity>
    </gnc:account>
    <gnc:transaction>
      <trn:id type="guid">tx-1</trn:id>
      <trn:currency>
        <cmdty:space>CURRENCY</cmdty:space>
        <cmdty:id>INR</cmdty:id>
      </trn:currency>
      <trn:date-posted>
        <ts:date>2026-11-16 10:00:00 +0000</ts:date>
      </trn:date-posted>
      <trn:description>AT&amp;T &lt;Bill&gt;</trn:description>
      <trn:slots>
        <slot>
          <slot:key>notes</slot:key>
          <slot:value type="string">Payment for 5G &amp; Fiber</slot:value>
        </slot>
      </trn:slots>
      <trn:splits>
        <trn:split>
          <split:id type="guid">sp-1</split:id>
          <split:reconciled-state>n</split:reconciled-state>
          <split:value>250000/100</split:value>
          <split:quantity>250000/100</split:quantity>
          <split:account type="guid">acc-1</split:account>
          <split:memo>Groceries &amp; Treats</split:memo>
        </trn:split>
        <trn:split>
          <split:id type="guid">sp-2</split:id>
          <split:reconciled-state>n</split:reconciled-state>
          <split:value>-250000/100</split:value>
          <split:quantity>-250000/100</split:quantity>
          <split:account type="guid">acc-2</split:account>
        </trn:split>
      </trn:splits>
    </gnc:transaction>
  </gnc:book>
</gnc-v2>"#;

    let boundary = "---------------------------974767299852498929531610575";
    let body_data = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"book.gnucash\"\r\nContent-Type: application/xml\r\n\r\n{xml}\r\n--{boundary}--\r\n"
    );

    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/import/gnucash-sqlite")
                .header("authorization", format!("Bearer {}", token))
                .header("content-type", format!("multipart/form-data; boundary={boundary}"))
                .body(Body::from(body_data))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    // Verify entities in accounts, transactions, and splits (IMP-1, IMP-7, SCOPE-1)
    let acc_name: String = sqlx::query_scalar("SELECT name FROM accounts WHERE user_id = $1 AND name LIKE 'Food%'")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(acc_name, "Food & Dining");

    let (desc, notes): (String, Option<String>) = sqlx::query_as("SELECT description, notes FROM transactions WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(desc, "AT&T <Bill>");
    assert_eq!(notes.as_deref(), Some("Payment for 5G & Fiber"));

    let memo: Option<String> = sqlx::query_scalar(
        "SELECT s.memo FROM splits s JOIN transactions t ON s.transaction_id = t.id WHERE t.user_id = $1 AND s.memo IS NOT NULL"
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(memo.as_deref(), Some("Groceries & Treats"));
}

#[tokio::test]
async fn test_issue_39_reject_orphan_and_root_splits() {
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("user_orphan_{}", user_id.simple());
    let email = format!("user_orphan_{}@example.com", user_id.simple());

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, token_version) VALUES ($1, $2, $3, 'hash', 0)"
    )
    .bind(user_id)
    .bind(&username)
    .bind(&email)
    .execute(&pool)
    .await
    .unwrap();

    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    let upload_sqlite = |bytes: Vec<u8>| {
        let pool = pool.clone();
        let token = token.clone();
        async move {
            let boundary = "---------------------------974767299852498929531610575";
            let mut body_bytes = Vec::new();
            body_bytes.extend_from_slice(
                format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.gnucash\"\r\nContent-Type: application/x-sqlite3\r\n\r\n").as_bytes(),
            );
            body_bytes.extend_from_slice(&bytes);
            body_bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

            let app = create_app(pool);
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/import/gnucash-sqlite")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", format!("multipart/form-data; boundary={boundary}"))
                    .body(Body::from(body_bytes))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };

    // 1. Transaction split references root account -> must return HTTP 400 (ACC-2, IMP-4, IMP-10, IMP-11)
    let temp_db1 = tempfile::NamedTempFile::new().unwrap();
    let conn1 = rusqlite::Connection::open(temp_db1.path()).unwrap();
    conn1.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('root1', 'Root Account', 'ROOT', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', 'root1', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Checking', 'BANK', 'c1', 'a1', NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('tx_root_split', 'c1', '', '20261102120000', '20261102120000', 'Root Split Transaction');
        INSERT INTO splits VALUES ('s1', 'tx_root_split', 'root1', '', '', 'n', NULL, 1000, 100, 1000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 'tx_root_split', 'a2', '', '', 'n', NULL, -1000, 100, -1000, 100, NULL);
    "#).unwrap();

    let bytes1 = std::fs::read(temp_db1.path()).unwrap();
    let resp1 = upload_sqlite(bytes1).await;
    assert_eq!(resp1.status(), StatusCode::BAD_REQUEST);
    let body_bytes1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let json1: Value = serde_json::from_slice(&body_bytes1).unwrap();
    let err_msg1 = json1["error"].as_str().unwrap();

    assert!(err_msg1.contains("root1"), "Error must name root account GUID: {}", err_msg1);
    assert!(err_msg1.contains("Root Split Transaction"), "Error must name tx description: {}", err_msg1);
    assert!(err_msg1.contains("2026-11-02"), "Error must name post date: {}", err_msg1);
    assert!(err_msg1.contains("accounts: [") && (err_msg1.contains("Assets:Checking") || err_msg1.contains("Checking")), "Error must name the transaction's other accounts (IMP-11): {}", err_msg1);

    // 2. Transaction split references unknown account -> must return HTTP 400 (ACC-2, IMP-4, IMP-10, IMP-11)
    let temp_db2 = tempfile::NamedTempFile::new().unwrap();
    let conn2 = rusqlite::Connection::open(temp_db2.path()).unwrap();
    conn2.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Checking', 'BANK', 'c1', 'a1', NULL, NULL, 0, 0);

        INSERT INTO transactions VALUES ('tx_orphan_split', 'c1', '', '20261102120000', '20261102120000', 'Orphan Split Transaction');
        INSERT INTO splits VALUES ('s1', 'tx_orphan_split', 'missing_acc_guid', '', '', 'n', NULL, 1000, 100, 1000, 100, NULL);
        INSERT INTO splits VALUES ('s2', 'tx_orphan_split', 'a2', '', '', 'n', NULL, -1000, 100, -1000, 100, NULL);
    "#).unwrap();

    let bytes2 = std::fs::read(temp_db2.path()).unwrap();
    let resp2 = upload_sqlite(bytes2).await;
    assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);
    let body_bytes2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body_bytes2).unwrap();
    let err_msg2 = json2["error"].as_str().unwrap();

    assert!(err_msg2.contains("missing_acc_guid"), "Error must name missing account GUID: {}", err_msg2);
    assert!(err_msg2.contains("Orphan Split Transaction"), "Error must name tx description: {}", err_msg2);
    assert!(err_msg2.contains("accounts: [") && (err_msg2.contains("Assets:Checking") || err_msg2.contains("Checking")), "Error must name the transaction's other accounts (IMP-11): {}", err_msg2);

    // 3. Transactions with fewer than 2 splits are skipped, transactions with >=2 splits are imported (IMP-10, ACC-1)
    let temp_db3 = tempfile::NamedTempFile::new().unwrap();
    let conn3 = rusqlite::Connection::open(temp_db3.path()).unwrap();
    conn3.execute_batch(r#"
        CREATE TABLE accounts (guid TEXT PRIMARY KEY, name TEXT, account_type TEXT, commodity_guid TEXT, parent_guid TEXT, code TEXT, description TEXT, hidden INT, placeholder INT);
        CREATE TABLE transactions (guid TEXT PRIMARY KEY, currency_guid TEXT, num TEXT, post_date TEXT, enter_date TEXT, description TEXT);
        CREATE TABLE splits (guid TEXT PRIMARY KEY, tx_guid TEXT, account_guid TEXT, memo TEXT, action TEXT, reconcile_state TEXT, reconcile_date TEXT, value_num INT, value_denom INT, quantity_num INT, quantity_denom INT, lot_guid TEXT);
        CREATE TABLE commodities (guid TEXT PRIMARY KEY, namespace TEXT, mnemonic TEXT, fullname TEXT, cusip TEXT, fraction INT, quote_flag INT, quote_source TEXT);

        INSERT INTO commodities VALUES ('c1', 'CURRENCY', 'INR', 'INR', '', 100, 1, 'currency');
        INSERT INTO accounts VALUES ('a1', 'Assets', 'ASSET', 'c1', NULL, NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a2', 'Checking', 'BANK', 'c1', 'a1', NULL, NULL, 0, 0);
        INSERT INTO accounts VALUES ('a3', 'Expense', 'EXPENSE', 'c1', NULL, NULL, NULL, 0, 0);

        -- Tx with 1 split (must be skipped)
        INSERT INTO transactions VALUES ('tx_one_split', 'c1', '', '20261102120000', '20261102120000', 'Single Split Tx');
        INSERT INTO splits VALUES ('s1_single', 'tx_one_split', 'a2', '', '', 'n', NULL, 500, 100, 500, 100, NULL);

        -- Tx with 2 splits (valid)
        INSERT INTO transactions VALUES ('tx_valid_two', 'c1', '', '20261102120000', '20261102120000', 'Valid Two Split Tx');
        INSERT INTO splits VALUES ('s1_valid', 'tx_valid_two', 'a2', '', '', 'n', NULL, -1000, 100, -1000, 100, NULL);
        INSERT INTO splits VALUES ('s2_valid', 'tx_valid_two', 'a3', '', '', 'n', NULL, 1000, 100, 1000, 100, NULL);
    "#).unwrap();

    let bytes3 = std::fs::read(temp_db3.path()).unwrap();
    let resp3 = upload_sqlite(bytes3).await;
    assert_eq!(resp3.status(), StatusCode::OK);
    let body_bytes3 = resp3.into_body().collect().await.unwrap().to_bytes();
    let json3: Value = serde_json::from_slice(&body_bytes3).unwrap();
    assert_eq!(json3["skipped_transactions"], 1);
    assert_eq!(json3["transactions_imported"], 1);

    // Verify all stored transactions for this user have >= 2 splits
    let min_splits: Option<i64> = sqlx::query_scalar(
        "SELECT MIN(cnt) FROM (SELECT COUNT(*) as cnt FROM splits s JOIN transactions t ON s.transaction_id = t.id WHERE t.user_id = $1 GROUP BY t.id) sub"
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(min_splits, Some(2));
}

#[tokio::test]
async fn test_issue_26_export_investment_currency_namespace() {
    let pool = get_test_pool().await;
    let user_id = uuid::Uuid::new_v4();
    let username = format!("user_inv_exp_{}", user_id.simple());
    let email = format!("user_inv_exp_{}@example.com", user_id.simple());

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, token_version) VALUES ($1, $2, $3, 'hash', 0)"
    )
    .bind(user_id)
    .bind(&username)
    .bind(&email)
    .execute(&pool)
    .await
    .unwrap();

    let token = gnucash_backend::auth::create_jwt(user_id, &username, &email, 0).unwrap();

    // Bank account (INR, currency)
    let bank_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, currency, commodity_namespace, commodity_fraction, placeholder, user_id) VALUES ($1, 'Main Bank', 'bank', 'INR', 'CURRENCY', 100, false, $2)")
        .bind(bank_id).bind(user_id).execute(&pool).await.unwrap();

    // Mutual fund account (INF209K01VA3, commodity)
    let fund_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, name, account_type, currency, commodity_namespace, commodity_fraction, placeholder, user_id) VALUES ($1, 'Parag Parikh Flexi Cap', 'mutual', 'INF209K01VA3', 'FUND', 1000, false, $2)")
        .bind(fund_id).bind(user_id).execute(&pool).await.unwrap();

    // Tx 1: Fund split UUID sorts BEFORE bank split UUID
    let (s_fund_1, s_bank_1) = {
        let u1 = uuid::Uuid::new_v4();
        let u2 = uuid::Uuid::new_v4();
        if u1 < u2 { (u1, u2) } else { (u2, u1) }
    };
    let tx1_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, user_id, post_date, description) VALUES ($1, $2, '2026-11-01', 'Buy Fund First')")
        .bind(tx1_id).bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, 1000.00, 10.000000, 'n')")
        .bind(s_fund_1).bind(tx1_id).bind(fund_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, -1000.00, -1000.000000, 'n')")
        .bind(s_bank_1).bind(tx1_id).bind(bank_id).execute(&pool).await.unwrap();

    // Tx 2: Fund split UUID sorts AFTER bank split UUID
    let (s_bank_2, s_fund_2) = {
        let u3 = uuid::Uuid::new_v4();
        let u4 = uuid::Uuid::new_v4();
        if u3 < u4 { (u3, u4) } else { (u4, u3) }
    };
    let tx2_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO transactions (id, user_id, post_date, description) VALUES ($1, $2, '2026-11-02', 'Buy Fund Second')")
        .bind(tx2_id).bind(user_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, -1000.00, -1000.000000, 'n')")
        .bind(s_bank_2).bind(tx2_id).bind(bank_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO splits (id, transaction_id, account_id, amount, quantity, reconciled_state) VALUES ($1, $2, $3, 1000.00, 10.000000, 'n')")
        .bind(s_fund_2).bind(tx2_id).bind(fund_id).execute(&pool).await.unwrap();

    // Call export SQLite
    let app = create_app(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/export/gnucash-sqlite")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();

    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), &bytes).unwrap();
    let conn = rusqlite::Connection::open(temp_file.path()).unwrap();

    // Assert that every transaction's currency_guid joins to a commodity with namespace = 'CURRENCY' (INR) (INV-5, EXP-5, SCOPE-1)
    let non_currency_txs: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions t JOIN commodities c ON t.currency_guid = c.guid WHERE c.namespace != 'CURRENCY' OR c.mnemonic != 'INR'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(non_currency_txs, 0, "All transactions must use CURRENCY INR namespace");

    // Assert fund splits are written with quantity_num/quantity_denom representing 10 units (fraction 1000 -> 10000/1000)
    let mut stmt = conn.prepare("SELECT quantity_num, quantity_denom FROM splits WHERE account_guid = (SELECT guid FROM accounts WHERE name = 'Parag Parikh Flexi Cap')").unwrap();
    let mut rows = stmt.query([]).unwrap();
    while let Some(row) = rows.next().unwrap() {
        let q_num: i64 = row.get(0).unwrap();
        let q_denom: i64 = row.get(1).unwrap();
        assert_eq!(q_num as f64 / q_denom as f64, 10.0, "Quantity must be 10 units");
    }
}
