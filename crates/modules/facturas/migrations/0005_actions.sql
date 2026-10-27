ALTER TABLE facturas_drafts
    ADD invoice_number VARCHAR(19) NULL,
    ADD sar_snapshot_json JSON NULL,
    ADD company_snapshot_json JSON NULL,
    ADD deleted_at DATETIME(6) NULL,
    ADD UNIQUE KEY facturas_number_unique (invoice_number);

CREATE TABLE facturas_payments (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    invoice_id BIGINT UNSIGNED NOT NULL,
    payment_date DATE NOT NULL,
    amount DECIMAL(14,2) NOT NULL,
    method VARCHAR(120) NOT NULL,
    reference VARCHAR(180) NOT NULL,
    created_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    CONSTRAINT facturas_payment_invoice_fk FOREIGN KEY (invoice_id) REFERENCES facturas_drafts(id),
    CONSTRAINT facturas_payment_actor_fk FOREIGN KEY (created_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE facturas_command_receipts (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY (actor_id,key_hash),
    CONSTRAINT facturas_command_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE facturas_documents (
    object_key VARCHAR(250) NOT NULL PRIMARY KEY,
    invoice_id BIGINT UNSIGNED NOT NULL,
    invoice_version BIGINT UNSIGNED NOT NULL,
    kind VARCHAR(20) NOT NULL,
    filename VARCHAR(180) NOT NULL,
    sha256 BINARY(32) NOT NULL,
    byte_size INT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    CONSTRAINT facturas_document_invoice_fk FOREIGN KEY (invoice_id) REFERENCES facturas_drafts(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE facturas_document_deletions (
    object_key VARCHAR(250) NOT NULL PRIMARY KEY,
    available_at DATETIME(6) NOT NULL,
    attempts INT UNSIGNED NOT NULL DEFAULT 0
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
