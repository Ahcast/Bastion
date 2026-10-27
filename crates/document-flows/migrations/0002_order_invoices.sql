CREATE TABLE document_order_invoices (
 order_id BIGINT UNSIGNED PRIMARY KEY, invoice_id BIGINT UNSIGNED NOT NULL UNIQUE,
 snapshot_json JSON NOT NULL, result_json JSON NOT NULL, created_by BIGINT UNSIGNED NOT NULL, created_at DATETIME(6) NOT NULL,
 FOREIGN KEY(order_id) REFERENCES ordenes_orders(id), FOREIGN KEY(invoice_id) REFERENCES facturas_drafts(id),
 FOREIGN KEY(created_by) REFERENCES users(id)
);
CREATE TABLE document_order_invoice_receipts (
 actor_id BIGINT UNSIGNED NOT NULL, key_hash BINARY(32) NOT NULL, request_hash BINARY(32) NOT NULL,
 result_json JSON NOT NULL, PRIMARY KEY(actor_id,key_hash), FOREIGN KEY(actor_id) REFERENCES users(id)
);
