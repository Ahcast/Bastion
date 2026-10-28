use bastion_core::{CoreError, CoreService, Database};
use bastion_facturas::{InvoiceFilter, InvoiceInput, InvoiceService, PaymentInput, PaymentTerm};
use sqlx::mysql::MySqlPoolOptions;

fn input() -> InvoiceInput {
    serde_json::from_value(serde_json::json!({
        "client_name":"Cliente de prueba", "order_number":"ORD-ACCIONES", "invoice_date":"2026-10-22",
        "payment_term_type":"credito", "credit_days":30, "discount_type":"amount", "discount_value":"0",
        "notes":"NOTA PRIVADA QUE NO VA AL PDF", "public_notes":"Entrega de prueba", "expenses":[],
        "items":[{"description":"Equipo #text(\"no ejecutar\")", "quantity":"1", "unit_price":"100", "cost_price":"60", "discount_type":"amount", "discount_value":"0", "tax_type":"isv15"}]
    })).unwrap()
}
fn payment(amount: &str) -> PaymentInput {
    PaymentInput {
        amount: amount.into(),
        date: chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap(),
        method: "Transferencia".into(),
        reference: "REF-TEST".into(),
    }
}

#[tokio::test]
async fn mysql57_invoice_actions_numbering_payments_documents_and_preserved_origins() {
    let Ok(url) = std::env::var("BASTION_INVOICE_ACTIONS_TEST_URL") else {
        eprintln!(
            "Requires isolated invoice actions database and private document fixture; CI supplies them."
        );
        return;
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .unwrap();
    let db = Database::from_test_pool(pool.clone());
    db.migrate().await.unwrap();
    InvoiceService::migrate(&db).await.unwrap();
    let core = CoreService::new(db.clone(), "x".repeat(32), [1; 32]).unwrap();
    let admin = core.admin();
    let invoices = InvoiceService::new(db, admin.clone());
    sqlx::query("INSERT INTO persons(id,full_name) VALUES (1,'Administrador'),(2,'Lector'),(3,'Solo eliminar')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO roles(id,name) VALUES (2,'Lector'),(3,'Solo eliminar')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(id,person_id,role_id,email,status) VALUES (1,1,1,'invoice-admin@example.test','active'),(2,2,2,'invoice-reader@example.test','active'),(3,3,3,'invoice-delete@example.test','active')").execute(&pool).await.unwrap();
    for (role, capability) in [(2, "ver"), (3, "eliminar")] {
        sqlx::query("INSERT INTO role_permissions(role_id,module_id,scope_hash,scope_key,capability) VALUES (?,'facturas',UNHEX(SHA2('documentos',256)),'documentos',?)").bind(role).bind(capability).execute(&pool).await.unwrap();
    }
    invoices.synchronize_release(true).await.unwrap();
    admin.set_module_enabled(1, "facturas", true).await.unwrap();
    admin.set_core_setting(1,"company.profile",serde_json::json!({"name":"Empresa original","rtn":"RTN","email":"empresa@example.test","phone":"123","address":"Dirección"})).await.unwrap();
    let settings = admin.get_sar_settings(1).await.unwrap();
    admin
        .set_sar_settings(
            1,
            bastion_core::SarSettings {
                limit_range: "000-001-01-00000001 / 000- 001-01-00000005".into(),
                current_invoice: "000-001-01-00000000".into(),
                limit_date: Some(chrono::NaiveDate::from_ymd_opt(9999, 12, 31).unwrap()),
                sar_text: "CAI de prueba".into(),
            },
            &settings.version,
        )
        .await
        .unwrap();
    let a = invoices
        .create(1, "invoice-actions-create-a", input())
        .await
        .unwrap();
    assert!(a.invoice_number.is_none());
    assert!(matches!(
        invoices.document(2, a.id, 1, "receipt").await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        invoices
            .mark_sent(2, a.id, 1, "invoice-actions-denied")
            .await,
        Err(CoreError::Forbidden)
    ));
    let mut fiscal_input = input();
    fiscal_input.purchase_order_exempt_number = "OC-EXENTA-123".into();
    fiscal_input.registration_certificate_number = "CONST-456".into();
    fiscal_input.sag_registration_number = "SAG-789".into();
    let b = invoices
        .create(1, "invoice-actions-create-b", fiscal_input)
        .await
        .unwrap();
    assert_eq!(b.data.purchase_order_exempt_number, "OC-EXENTA-123");
    assert_eq!(b.data.registration_certificate_number, "CONST-456");
    assert_eq!(b.data.sag_registration_number, "SAG-789");
    assert_eq!(b.calculation.totals.total, "100.00");
    let (sent_a, sent_b) = tokio::join!(
        invoices.mark_sent(1, a.id, 1, "invoice-actions-send-a"),
        invoices.mark_sent(1, b.id, 1, "invoice-actions-send-b")
    );
    let sent_a = sent_a.unwrap();
    let sent_b = sent_b.unwrap();
    assert_ne!(sent_a.invoice_number, sent_b.invoice_number);
    assert_eq!(sent_a.status, "Enviado");
    assert_eq!(sent_a.version, 2);
    assert_eq!(
        invoices
            .mark_sent(1, a.id, 1, "invoice-actions-send-a")
            .await
            .unwrap()
            .invoice_number,
        sent_a.invoice_number
    );
    let mut updated_input = sent_b.data.clone();
    updated_input.public_notes = "Actualizada sin pago".into();
    let edited_b = invoices.replace(1, b.id, 2, updated_input).await.unwrap();
    assert_eq!(edited_b.status, "Enviado");
    assert_eq!(edited_b.invoice_number, sent_b.invoice_number);
    assert_eq!(edited_b.version, 3);
    assert_eq!(
        serde_json::to_value(&edited_b.sar_snapshot).unwrap(),
        serde_json::to_value(&sent_b.sar_snapshot).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&edited_b.company_snapshot).unwrap(),
        serde_json::to_value(&sent_b.company_snapshot).unwrap()
    );
    assert!(matches!(
        invoices
            .delete(1, a.id, 2, "invoice-actions-delete-issued")
            .await,
        Err(CoreError::Conflict)
    ));
    let settings = admin.get_sar_settings(1).await.unwrap();
    let mut reset = settings.settings.clone();
    reset.current_invoice = "000-001-01-00000000".into();
    assert!(matches!(
        admin.set_sar_settings(1, reset, &settings.version).await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        invoices
            .add_payment(1, a.id, 2, "invoice-actions-overpay", payment("116"))
            .await,
        Err(CoreError::Invalid)
    ));
    let (paid_a, paid_b) = tokio::join!(
        invoices.add_payment(1, a.id, 2, "invoice-actions-payment-1", payment("40")),
        invoices.add_payment(1, a.id, 2, "invoice-actions-payment-1", payment("40"))
    );
    let paid = paid_a.unwrap();
    assert_eq!(paid_b.unwrap().payments.len(), 1);
    assert_eq!(paid.calculation.balance, "75.00");
    assert_eq!(paid.status, "Enviado");
    assert!(matches!(
        invoices
            .add_payment(1, a.id, 2, "invoice-actions-payment-1", payment("41"))
            .await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        invoices
            .add_payment(1, a.id, 2, "invoice-actions-payment-stale", payment("10"))
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    let paid = invoices
        .add_payment(1, a.id, 3, "invoice-actions-payment-2", payment("75"))
        .await
        .unwrap();
    assert_eq!(paid.status, "Pagado");
    assert_eq!(paid.calculation.balance, "0.00");
    assert_eq!(paid.calculation.paid_total, "115.00");
    assert_eq!(paid.payments.len(), 2);
    assert!(matches!(
        invoices.replace(1, a.id, 4, input()).await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        invoices
            .add_payment(1, a.id, 4, "invoice-actions-payment-3", payment("1"))
            .await,
        Err(CoreError::Conflict)
    ));
    let clone = invoices
        .clone_invoice(1, a.id, 4, "invoice-actions-clone")
        .await
        .unwrap();
    assert_eq!(clone.status, "Borrador");
    assert!(clone.invoice_number.is_none());
    assert!(clone.payments.is_empty());
    assert_eq!(clone.calculation.balance, "115.00");
    assert!(clone.source_quote.is_none() && clone.source_order.is_none());
    assert_eq!(
        invoices
            .clone_invoice(1, a.id, 4, "invoice-actions-clone")
            .await
            .unwrap()
            .id,
        clone.id
    );
    // Deleting archives without removing origin metadata or reusing a number.
    sqlx::query("UPDATE facturas_drafts SET source_quote_json=? WHERE id=?")
        .bind(serde_json::json!({"id":999,"version":1,"number":"COT-ORIGEN"}))
        .bind(clone.id)
        .execute(&pool)
        .await
        .unwrap();
    invoices
        .delete(3, clone.id, 1, "invoice-actions-delete-draft")
        .await
        .unwrap();
    invoices
        .delete(3, clone.id, 1, "invoice-actions-delete-draft")
        .await
        .unwrap();
    assert!(matches!(
        invoices.detail(1, clone.id).await,
        Err(CoreError::NotFound)
    ));
    let origin: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT source_quote_json FROM facturas_drafts WHERE id=? AND deleted_at IS NOT NULL",
    )
    .bind(clone.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(origin.is_some());
    assert!(
        invoices
            .list(1, &InvoiceFilter::default())
            .await
            .unwrap()
            .items
            .iter()
            .all(|row| row.id != clone.id)
    );
    let mut cash = input();
    cash.payment_term_type = PaymentTerm::Contado;
    cash.initial_payment = Some(payment("115"));
    let cash = invoices
        .create(1, "invoice-actions-cash", cash)
        .await
        .unwrap();
    assert_eq!(cash.payments.len(), 1);
    assert_eq!(cash.calculation.balance, "0.00");
    assert_eq!(
        invoices
            .mark_sent(1, cash.id, 1, "invoice-actions-send-cash")
            .await
            .unwrap()
            .status,
        "Pagado"
    );
    let cash_clone = invoices
        .clone_invoice(1, cash.id, 2, "invoice-actions-cash-clone")
        .await
        .unwrap();
    assert!(matches!(
        invoices
            .mark_sent(1, cash_clone.id, 1, "invoice-actions-cash-unpaid")
            .await,
        Err(CoreError::Invalid)
    ));
    let mut paid_input = cash_clone.data;
    paid_input.initial_payment = Some(payment("115"));
    let cash_clone = invoices
        .replace(1, cash_clone.id, 1, paid_input)
        .await
        .unwrap();
    assert_eq!(cash_clone.payments.len(), 1);
    let date = admin.get_sar_settings(1).await.unwrap();
    let mut expired = date.settings.clone();
    expired.limit_date = Some(chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
    admin
        .set_sar_settings(1, expired, &date.version)
        .await
        .unwrap();
    assert!(matches!(
        invoices
            .mark_sent(1, cash_clone.id, 2, "invoice-actions-expired")
            .await,
        Err(CoreError::Conflict)
    ));
    assert_eq!(invoices.detail(1, cash_clone.id).await.unwrap().version, 2);
    let current = admin.get_sar_settings(1).await.unwrap();
    let mut restored = current.settings;
    restored.limit_date = Some(chrono::NaiveDate::from_ymd_opt(9999, 12, 31).unwrap());
    admin
        .set_sar_settings(1, restored, &current.version)
        .await
        .unwrap();
    admin
        .set_core_setting(
            1,
            "company.profile",
            serde_json::json!({"name":"Empresa nueva"}),
        )
        .await
        .unwrap();
    assert_eq!(
        invoices
            .detail(1, a.id)
            .await
            .unwrap()
            .company_snapshot
            .unwrap()
            .name,
        "Empresa original"
    );
    for kind in ["invoice", "delivery", "receipt"] {
        let doc = invoices.document(2, a.id, 4, kind).await.unwrap();
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(doc.content_base64)
            .unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(doc.filename.ends_with(".pdf"));
        invoices.document(2, a.id, 4, kind).await.unwrap();
    }
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM facturas_documents WHERE invoice_id=?")
            .bind(a.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 3);
    let before_receipt = invoices.detail(2, b.id).await.unwrap();
    assert!(before_receipt.payments.is_empty());
    let receipt = invoices.document(2, b.id, 3, "receipt").await.unwrap();
    assert!(!receipt.content_base64.is_empty());
    let after_receipt = invoices.detail(2, b.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(before_receipt).unwrap(),
        serde_json::to_value(after_receipt).unwrap()
    );
    let partially_paid_b = invoices
        .add_payment(1, b.id, 3, "invoice-actions-b-payment", payment("20"))
        .await
        .unwrap();
    let mut revised = partially_paid_b.data.clone();
    revised.items[0].unit_price = "90".into();
    let preview = invoices.preview_saved(1, b.id, 4, &revised).await.unwrap();
    assert_eq!(preview.paid_total, "20.00");
    assert_eq!(preview.balance, "70.00");
    let edited_with_payment = invoices.replace(1, b.id, 4, revised.clone()).await.unwrap();
    assert_eq!(edited_with_payment.invoice_number, sent_b.invoice_number);
    assert_eq!(edited_with_payment.payments.len(), 1);
    assert_eq!(edited_with_payment.calculation.balance, "70.00");
    revised.items[0].unit_price = "10".into();
    assert!(matches!(
        invoices.replace(1, b.id, 5, revised).await,
        Err(CoreError::Invalid)
    ));
    assert!(matches!(
        invoices
            .replace(1, b.id, 4, edited_with_payment.data.clone())
            .await,
        Err(CoreError::PreconditionFailed)
    ));
    let mut fully_covered = edited_with_payment.data.clone();
    fully_covered.items[0].unit_price = "20".into();
    let paid_by_revision = invoices.replace(1, b.id, 5, fully_covered).await.unwrap();
    assert_eq!(paid_by_revision.status, "Pagado");
    assert_eq!(paid_by_revision.calculation.balance, "0.00");
    assert_eq!(paid_by_revision.invoice_number, sent_b.invoice_number);
    assert!(matches!(
        invoices.replace(1, b.id, 6, paid_by_revision.data).await,
        Err(CoreError::Conflict)
    ));
    assert!(matches!(
        invoices.document(2, a.id, 3, "invoice").await,
        Err(CoreError::PreconditionFailed)
    ));
    let audits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE module_id='facturas' AND action='invoice.payment' AND target_id=?").bind(a.id.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(audits, 2);
    let setting = admin.get_sar_settings(1).await.unwrap();
    assert_eq!(
        setting.settings.current_invoice[11..]
            .parse::<u32>()
            .unwrap(),
        3
    );
    let issued = invoices
        .mark_sent(1, cash_clone.id, 2, "invoice-actions-fourth")
        .await
        .unwrap();
    assert!(issued.invoice_number.unwrap().ends_with("00000004"));
    let fifth = invoices
        .create(1, "invoice-actions-fifth-create", input())
        .await
        .unwrap();
    invoices
        .mark_sent(1, fifth.id, 1, "invoice-actions-fifth-send")
        .await
        .unwrap();
    let exhausted = invoices
        .create(1, "invoice-actions-sixth-create", input())
        .await
        .unwrap();
    assert!(matches!(
        invoices
            .mark_sent(1, exhausted.id, 1, "invoice-actions-exhausted")
            .await,
        Err(CoreError::Conflict)
    ));
    assert!(
        invoices
            .detail(1, exhausted.id)
            .await
            .unwrap()
            .invoice_number
            .is_none()
    );
    assert!(
        invoices
            .document(1, exhausted.id, 1, "invoice")
            .await
            .is_ok()
    );
    admin
        .set_module_enabled(1, "facturas", false)
        .await
        .unwrap();
    assert!(matches!(
        invoices.document(1, a.id, 4, "invoice").await,
        Err(CoreError::NotFound)
    ));
}
