-- New Bastion expense records only; no Legacy data or order association is imported.
CREATE TABLE gastos_records (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    version BIGINT UNSIGNED NOT NULL DEFAULT 1,
    expense_date DATE NOT NULL,
    invoice_number VARCHAR(120) NOT NULL,
    provider VARCHAR(255) NOT NULL,
    currency CHAR(3) NOT NULL,
    taxable_amount DECIMAL(14,2) NOT NULL,
    exempt_amount DECIMAL(14,2) NOT NULL,
    isv_15 DECIMAL(14,2) NOT NULL,
    total DECIMAL(14,2) NOT NULL,
    created_by BIGINT UNSIGNED NOT NULL,
    updated_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    updated_at DATETIME(6) NOT NULL,
    KEY gastos_date (expense_date,id),

    CONSTRAINT gastos_creator_fk FOREIGN KEY (created_by) REFERENCES users(id),
    CONSTRAINT gastos_updater_fk FOREIGN KEY (updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE gastos_create_receipts (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY (actor_id,key_hash),
    CONSTRAINT gastos_receipt_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE gastos_actor_limits (
    actor_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    window_start DATETIME(6) NOT NULL,
    requests INT UNSIGNED NOT NULL,
    CONSTRAINT gastos_limit_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
