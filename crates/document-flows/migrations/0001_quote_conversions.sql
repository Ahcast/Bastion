CREATE TABLE document_quote_conversions (
 id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
 quote_id BIGINT UNSIGNED NULL,
 source_id BIGINT UNSIGNED NOT NULL,
 invoice_source_id BIGINT UNSIGNED NULL,
 invoice_id BIGINT UNSIGNED NULL,
 order_id BIGINT UNSIGNED NULL,
 snapshot_json JSON NOT NULL,
 result_json JSON NOT NULL,
 created_by BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 UNIQUE KEY document_single_quote_invoice (invoice_source_id),
 UNIQUE KEY document_invoice_origin (invoice_id),
 UNIQUE KEY document_order_origin (order_id),
 KEY document_quote_origin (source_id),
 FOREIGN KEY (quote_id) REFERENCES cotizaciones_drafts(id) ON DELETE SET NULL,
 FOREIGN KEY (invoice_id) REFERENCES facturas_drafts(id),
 FOREIGN KEY (order_id) REFERENCES ordenes_orders(id),
 FOREIGN KEY (created_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE document_conversion_receipts (
 actor_id BIGINT UNSIGNED NOT NULL,
 key_hash BINARY(32) NOT NULL,
 request_hash BINARY(32) NOT NULL,
 conversion_id BIGINT UNSIGNED NOT NULL,
 result_json JSON NOT NULL,
 PRIMARY KEY (actor_id,key_hash),
 FOREIGN KEY (actor_id) REFERENCES users(id),
 FOREIGN KEY (conversion_id) REFERENCES document_quote_conversions(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

