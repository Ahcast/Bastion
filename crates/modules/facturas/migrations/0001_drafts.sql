-- New Bastion drafts only. No fiscal numbering, issued invoices or payments.
CREATE TABLE facturas_drafts (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    version BIGINT UNSIGNED NOT NULL DEFAULT 1,
    status VARCHAR(20) NOT NULL DEFAULT 'Borrador',
    client_name VARCHAR(255) NOT NULL,
    order_number VARCHAR(100) NOT NULL,
    invoice_date DATE NOT NULL,
    data_json JSON NOT NULL,
    calculation_json JSON NOT NULL,
    created_by BIGINT UNSIGNED NOT NULL,
    updated_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    updated_at DATETIME(6) NOT NULL,
    CONSTRAINT facturas_creator_fk FOREIGN KEY (created_by) REFERENCES users(id),
    CONSTRAINT facturas_updater_fk FOREIGN KEY (updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE facturas_create_receipts (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY (actor_id, key_hash),
    CONSTRAINT facturas_receipt_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE facturas_actor_limits (
    actor_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    window_start DATETIME(6) NOT NULL,
    requests INT UNSIGNED NOT NULL,
    CONSTRAINT facturas_limit_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
