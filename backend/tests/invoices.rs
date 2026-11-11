mod common;

use backend::{db, routes::invoices as inv_routes, tasks::advance_recur_date};
use uuid::Uuid;

fn base_invoice<'a>(
    id: &'a str,
    client_id: &'a str,
    number: &'a str,
) -> db::invoices::NewInvoice<'a> {
    db::invoices::NewInvoice {
        id,
        client_id,
        number,
        currency: "KES",
        terms: "Net 30",
        issued_date: "2025-01-01",
        due_date: "2025-01-31",
        project_type: "IT Support",
        project_location: "Remote",
        billed_to_name: "Acme Corp",
        billed_to_role: "Client",
        billed_to_addr1: "123 Main St",
        billed_to_addr2: "",
        billed_to_pin: "",
        billed_to_email: "billing@acme.example",
        billed_to_phone: "+254-700-000000",
        items_json: r#"[{"name":"Support hour","sub":null,"qty":2,"rate":5000}]"#,
        notes_json: r#"[{"k":"PO","v":"PO-2025-001"}]"#,
        editor_note: "Thank you for your business.",
        kra_number: None,
        tax_rate: 0.0,
        recurring: false,
        recur_interval: None,
        next_recur_date: None,
        sub_client_id: None,
    }
}

// ── Create ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_invoice_defaults_to_draft_status() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let id = Uuid::new_v4().to_string();

    let invoice = db::invoices::create(&pool, base_invoice(&id, &client_id, "INV-001"))
        .await
        .unwrap();

    assert_eq!(invoice.status, "draft");
}

#[tokio::test]
async fn create_invoice_persists_all_fields() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let id = Uuid::new_v4().to_string();

    db::invoices::create(&pool, base_invoice(&id, &client_id, "INV-002"))
        .await
        .unwrap();

    let fetched = db::invoices::find_by_id(&pool, &id).await.unwrap().unwrap();
    assert_eq!(fetched.number, "INV-002");
    assert_eq!(fetched.currency, "KES");
    assert_eq!(fetched.billed_to_name, "Acme Corp");
    assert_eq!(fetched.billed_to_email, "billing@acme.example");
    assert_eq!(fetched.billed_to_phone, "+254-700-000000");
    assert_eq!(fetched.editor_note, "Thank you for your business.");
    assert_eq!(fetched.client_id, Some(client_id));
}

// ── List ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_invoices_returns_all() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let id1 = Uuid::new_v4().to_string();
    let id2 = Uuid::new_v4().to_string();
    db::invoices::create(&pool, base_invoice(&id1, &client_id, "INV-A"))
        .await
        .unwrap();
    db::invoices::create(&pool, base_invoice(&id2, &client_id, "INV-B"))
        .await
        .unwrap();

    let all = db::invoices::list(&pool).await.unwrap();
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn list_for_client_filters_by_client() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_a, _, _) = common::create_test_client(&pool).await;
    let (client_b, _, _) = common::create_test_client(&pool).await;

    let id_a = Uuid::new_v4().to_string();
    let id_b = Uuid::new_v4().to_string();
    db::invoices::create(&pool, base_invoice(&id_a, &client_a, "INV-A"))
        .await
        .unwrap();
    db::invoices::create(&pool, base_invoice(&id_b, &client_b, "INV-B"))
        .await
        .unwrap();

    let for_a = db::invoices::list_for_client(&pool, &client_a)
        .await
        .unwrap();
    assert_eq!(for_a.len(), 1);
    assert_eq!(for_a[0].client_id, Some(client_a));
}

// ── Update ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn update_invoice_changes_status_and_number() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let id = Uuid::new_v4().to_string();

    db::invoices::create(&pool, base_invoice(&id, &client_id, "INV-001"))
        .await
        .unwrap();

    db::invoices::update(
        &pool,
        &id,
        db::invoices::UpdateInvoice {
            number: "INV-001-REVISED",
            status: "sent",
            currency: "KES",
            terms: "Net 30",
            issued_date: "2025-01-01",
            due_date: "2025-01-31",
            project_type: "IT Support",
            project_location: "Remote",
            billed_to_name: "Acme Corp",
            billed_to_role: "Client",
            billed_to_addr1: "123 Main St",
            billed_to_addr2: "",
            billed_to_pin: "",
            billed_to_email: "billing@acme.example",
            billed_to_phone: "+254-700-000000",
            items_json: r#"[{"name":"Support hour","sub":null,"qty":2,"rate":5000}]"#,
            notes_json: "[]",
            editor_note: "",
            kra_number: None,
            tax_rate: 0.0,
            recurring: false,
            recur_interval: None,
            next_recur_date: None,
            sub_client_id: None,
        },
    )
    .await
    .unwrap();

    let updated = db::invoices::find_by_id(&pool, &id).await.unwrap().unwrap();
    assert_eq!(updated.number, "INV-001-REVISED");
    assert_eq!(updated.status, "sent");
}

// ── Delete ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_invoice_removes_from_db() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let id = Uuid::new_v4().to_string();

    db::invoices::create(&pool, base_invoice(&id, &client_id, "INV-DEL"))
        .await
        .unwrap();

    db::invoices::delete(&pool, &id).await.unwrap();

    let gone = db::invoices::find_by_id(&pool, &id).await.unwrap();
    assert!(gone.is_none(), "invoice must not exist after delete");
}

// ── Sequence number ───────────────────────────────────────────────────────────

#[tokio::test]
async fn next_seq_increments_with_each_invoice() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;

    assert_eq!(db::invoices::next_seq(&pool).await.unwrap(), 1);

    let id = Uuid::new_v4().to_string();
    db::invoices::create(&pool, base_invoice(&id, &client_id, "INV-001"))
        .await
        .unwrap();

    assert_eq!(db::invoices::next_seq(&pool).await.unwrap(), 2);
}

// ── VAT / tax_rate ────────────────────────────────────────────────────────────

use backend::routes::invoices::compute_totals;

#[tokio::test]
async fn create_invoice_with_tax_rate_persists_and_roundtrips() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let id = Uuid::new_v4().to_string();

    let inv = db::invoices::create(
        &pool,
        db::invoices::NewInvoice {
            tax_rate: 16.0,
            ..base_invoice(&id, &client_id, "INV-VAT-01")
        },
    )
    .await
    .unwrap();

    assert_eq!(inv.tax_rate, 16.0);

    let fetched = db::invoices::find_by_id(&pool, &id).await.unwrap().unwrap();
    assert_eq!(fetched.tax_rate, 16.0);
}

#[tokio::test]
async fn create_invoice_without_tax_rate_defaults_to_zero() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let id = Uuid::new_v4().to_string();

    let inv = db::invoices::create(&pool, base_invoice(&id, &client_id, "INV-VAT-02"))
        .await
        .unwrap();

    assert_eq!(inv.tax_rate, 0.0);
}

#[test]
fn compute_totals_subtotal_1000_at_8_1_percent() {
    let (vat, grand) = compute_totals(1000, 8.1);
    assert_eq!(vat, 81);
    assert_eq!(grand, 1081);
}

#[test]
fn compute_totals_subtotal_999_at_16_percent() {
    // 999 * 0.16 = 159.84 — rounds to 160
    let (vat, grand) = compute_totals(999, 16.0);
    assert_eq!(vat, 160);
    assert_eq!(grand, 1159);
}

#[test]
fn compute_totals_zero_rate_returns_no_vat() {
    let (vat, grand) = compute_totals(5000, 0.0);
    assert_eq!(vat, 0);
    assert_eq!(grand, 5000);
}

#[test]
fn render_html_contains_vat_row_when_rate_set() {
    use backend::dto::{InvoiceItem, InvoiceResponse};
    use backend::models::DeskProfile;

    let desk = DeskProfile {
        id: "d1".into(),
        name: "Lodgr".into(),
        tagline: "Support".into(),
        email: "desk@lodgr.local".into(),
        website: "lodgr.local".into(),
        city: "Nairobi".into(),
        phone: "".into(),
        vat_number: "".into(),
    };
    let inv = InvoiceResponse {
        id: "i1".into(),
        client_id: None,
        number: "INV-001".into(),
        status: "draft".into(),
        currency: "KES".into(),
        terms: "Net 14".into(),
        issued_date: "2025-01-01".into(),
        due_date: "2025-01-31".into(),
        project_type: "IT".into(),
        project_location: "Nairobi".into(),
        billed_to_name: "Acme".into(),
        billed_to_role: "".into(),
        billed_to_addr1: "".into(),
        billed_to_addr2: "".into(),
        billed_to_pin: "".into(),
        billed_to_email: "".into(),
        billed_to_phone: "".into(),
        items: vec![InvoiceItem {
            name: "Service".into(),
            sub: None,
            qty: 1,
            rate: 10000,
        }],
        notes: vec![],
        editor_note: "".into(),
        kra_number: None,
        tax_rate: 16.0,
        recurring: false,
        recur_interval: None,
        next_recur_date: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        sub_client_id: None,
        sub_client_name: None,
    };

    let html = backend::routes::invoices::render_invoice_html_pub(&inv, &desk);
    assert!(html.contains("VAT (16%)"), "HTML must contain VAT label");
    assert!(html.contains("1,600"), "HTML must show VAT amount 1600");
    assert!(html.contains("11,600"), "HTML must show grand total 11600");
}

#[test]
fn render_html_no_vat_row_when_rate_zero() {
    use backend::dto::{InvoiceItem, InvoiceResponse};
    use backend::models::DeskProfile;

    let desk = DeskProfile {
        id: "d1".into(),
        name: "Lodgr".into(),
        tagline: "Support".into(),
        email: "desk@lodgr.local".into(),
        website: "lodgr.local".into(),
        city: "Nairobi".into(),
        phone: "".into(),
        vat_number: "".into(),
    };
    let inv = InvoiceResponse {
        id: "i1".into(),
        client_id: None,
        number: "INV-002".into(),
        status: "draft".into(),
        currency: "KES".into(),
        terms: "Net 14".into(),
        issued_date: "2025-01-01".into(),
        due_date: "2025-01-31".into(),
        project_type: "IT".into(),
        project_location: "Nairobi".into(),
        billed_to_name: "Acme".into(),
        billed_to_role: "".into(),
        billed_to_addr1: "".into(),
        billed_to_addr2: "".into(),
        billed_to_pin: "".into(),
        billed_to_email: "".into(),
        billed_to_phone: "".into(),
        items: vec![InvoiceItem {
            name: "Service".into(),
            sub: None,
            qty: 1,
            rate: 10000,
        }],
        notes: vec![],
        editor_note: "".into(),
        kra_number: None,
        tax_rate: 0.0,
        recurring: false,
        recur_interval: None,
        next_recur_date: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        sub_client_id: None,
        sub_client_name: None,
    };

    let html = backend::routes::invoices::render_invoice_html_pub(&inv, &desk);
    assert!(
        !html.contains("VAT"),
        "HTML must not contain VAT row for 0% rate"
    );
    assert!(html.contains("10,000"), "HTML must show plain total 10000");
}

// ── Recurring due date & number ───────────────────────────────────────────────

#[test]
fn advance_recur_date_monthly_adds_one_month() {
    let next = advance_recur_date("2025-01-31", "monthly").unwrap();
    assert_eq!(next, "2025-02-28", "monthly from Jan 31 → Feb 28");
}

#[test]
fn advance_recur_date_quarterly_adds_three_months() {
    let next = advance_recur_date("2025-01-01", "quarterly").unwrap();
    assert_eq!(next, "2025-04-01");
}

#[test]
fn advance_recur_date_yearly_adds_twelve_months() {
    let next = advance_recur_date("2025-03-15", "yearly").unwrap();
    assert_eq!(next, "2026-03-15");
}

#[test]
fn advance_recur_date_invalid_input_returns_none() {
    assert!(advance_recur_date("not-a-date", "monthly").is_none());
}

#[tokio::test]
async fn next_seq_tx_matches_pool_next_seq() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let pool_seq = db::invoices::next_seq(&pool).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let tx_seq = db::invoices::next_seq_tx(&mut tx).await.unwrap();
    tx.rollback().await.unwrap();

    assert_eq!(pool_seq, tx_seq, "tx variant must yield the same seq as pool variant");

    // Create one invoice and verify both variants see it.
    let id = Uuid::new_v4().to_string();
    db::invoices::create(&pool, base_invoice(&id, &client_id, "INV-0001"))
        .await
        .unwrap();

    let pool_seq2 = db::invoices::next_seq(&pool).await.unwrap();
    let mut tx2 = pool.begin().await.unwrap();
    let tx_seq2 = db::invoices::next_seq_tx(&mut tx2).await.unwrap();
    tx2.rollback().await.unwrap();

    assert_eq!(pool_seq2, tx_seq2);
    assert_eq!(pool_seq2, 2);
}

#[tokio::test]
async fn create_tx_stores_invoice_with_correct_number() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let new_id = Uuid::new_v4().to_string();

    let mut tx = pool.begin().await.unwrap();
    let seq = db::invoices::next_seq_tx(&mut tx).await.unwrap();
    let number = format!("INV-{seq:04}");
    db::invoices::create_tx(
        &mut tx,
        base_invoice(&new_id, &client_id, &number),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let stored = db::invoices::find_by_id(&pool, &new_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.number, "INV-0001");
}

// ── Clearing fields via update ────────────────────────────────────────────────

fn update_all(inv: &backend::models::Invoice) -> db::invoices::UpdateInvoice<'_> {
    db::invoices::UpdateInvoice {
        number: &inv.number,
        status: &inv.status,
        currency: &inv.currency,
        terms: &inv.terms,
        issued_date: &inv.issued_date,
        due_date: &inv.due_date,
        project_type: &inv.project_type,
        project_location: &inv.project_location,
        billed_to_name: &inv.billed_to_name,
        billed_to_role: &inv.billed_to_role,
        billed_to_addr1: &inv.billed_to_addr1,
        billed_to_addr2: &inv.billed_to_addr2,
        billed_to_pin: &inv.billed_to_pin,
        billed_to_email: &inv.billed_to_email,
        billed_to_phone: &inv.billed_to_phone,
        items_json: &inv.items,
        notes_json: &inv.notes,
        editor_note: &inv.editor_note,
        kra_number: inv.kra_number.as_deref(),
        tax_rate: inv.tax_rate,
        recurring: inv.recurring != 0,
        recur_interval: inv.recur_interval.as_deref(),
        next_recur_date: inv.next_recur_date.as_deref(),
        sub_client_id: inv.sub_client_id.as_deref(),
    }
}

#[tokio::test]
async fn update_clears_kra_number_when_set_to_none() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let id = Uuid::new_v4().to_string();

    // Create with a kra_number.
    db::invoices::create(
        &pool,
        db::invoices::NewInvoice {
            kra_number: Some("KRA-99"),
            ..base_invoice(&id, &client_id, "INV-KRA-01")
        },
    )
    .await
    .unwrap();

    let inv = db::invoices::find_by_id(&pool, &id).await.unwrap().unwrap();
    assert_eq!(inv.kra_number.as_deref(), Some("KRA-99"));

    // Now clear it by passing None.
    let mut u = update_all(&inv);
    u.kra_number = None;
    db::invoices::update(&pool, &id, u).await.unwrap();

    let updated = db::invoices::find_by_id(&pool, &id).await.unwrap().unwrap();
    assert!(updated.kra_number.is_none(), "kra_number must be NULL after clearing");
}

#[tokio::test]
async fn update_clears_recur_fields_when_recurring_false() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let id = Uuid::new_v4().to_string();

    // Create a recurring template.
    db::invoices::create(
        &pool,
        db::invoices::NewInvoice {
            recurring: true,
            recur_interval: Some("monthly"),
            next_recur_date: Some("2025-02-01"),
            ..base_invoice(&id, &client_id, "INV-REC-01")
        },
    )
    .await
    .unwrap();

    let inv = db::invoices::find_by_id(&pool, &id).await.unwrap().unwrap();
    assert_eq!(inv.recurring, 1);

    // Turn off recurring — route handler clears recur_interval and next_recur_date.
    let mut u = update_all(&inv);
    u.recurring = false;
    u.recur_interval = None;
    u.next_recur_date = None;
    db::invoices::update(&pool, &id, u).await.unwrap();

    let updated = db::invoices::find_by_id(&pool, &id).await.unwrap().unwrap();
    assert_eq!(updated.recurring, 0);
    assert!(updated.recur_interval.is_none(), "recur_interval must be NULL when recurring=false");
    assert!(updated.next_recur_date.is_none(), "next_recur_date must be NULL when recurring=false");
}

// ── Status / transition validation ───────────────────────────────────────────

#[test]
fn valid_status_accepts_known_values() {
    for s in ["draft", "sent", "paid", "void"] {
        assert!(inv_routes::valid_status(s), "{s} must be a valid status");
    }
}

#[test]
fn valid_status_rejects_unknown_values() {
    for s in ["pending", "cancelled", "DRAFT", ""] {
        assert!(!inv_routes::valid_status(s), "{s} must not be a valid status");
    }
}

#[test]
fn valid_transition_allows_documented_paths() {
    let allowed = [
        ("draft", "sent"),
        ("sent", "draft"),
        ("sent", "paid"),
        ("sent", "void"),
        ("paid", "void"),
    ];
    for (from, to) in allowed {
        assert!(inv_routes::valid_transition(from, to), "{from}→{to} must be allowed");
    }
}

#[test]
fn valid_transition_rejects_undocumented_paths() {
    let blocked = [
        ("draft", "paid"),
        ("draft", "void"),
        ("paid", "draft"),
        ("paid", "sent"),
        ("void", "draft"),
        ("void", "sent"),
        ("void", "paid"),
    ];
    for (from, to) in blocked {
        assert!(!inv_routes::valid_transition(from, to), "{from}→{to} must be blocked");
    }
}

#[test]
fn valid_transition_no_op_same_status() {
    for s in ["draft", "sent", "paid", "void"] {
        assert!(inv_routes::valid_transition(s, s), "{s}→{s} no-op must be allowed");
    }
}

#[test]
fn valid_date_accepts_yyyy_mm_dd() {
    assert!(inv_routes::valid_date("2025-01-31"));
    assert!(inv_routes::valid_date("2024-02-29")); // leap year
}

#[test]
fn valid_date_rejects_bad_formats() {
    assert!(!inv_routes::valid_date("01/31/2025"));
    assert!(!inv_routes::valid_date("2025-13-01")); // invalid month
    assert!(!inv_routes::valid_date(""));
    assert!(!inv_routes::valid_date("2025-02-30")); // Feb 30 never exists
}
