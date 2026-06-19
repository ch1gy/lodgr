mod common;

use backend::db;
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
