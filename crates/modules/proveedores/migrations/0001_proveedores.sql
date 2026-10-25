-- New Bastion supplier catalog. Legacy records are not imported or modified.
CREATE TABLE proveedores_records (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    version BIGINT UNSIGNED NOT NULL DEFAULT 1,
    supplier_name VARCHAR(255) NOT NULL,
    user_contact VARCHAR(255) NOT NULL DEFAULT '',
    email VARCHAR(254) NOT NULL DEFAULT '',
    rtn VARCHAR(100) NOT NULL DEFAULT '',
    phone_number VARCHAR(60) NOT NULL DEFAULT '',
    mobile_number VARCHAR(60) NOT NULL DEFAULT '',
    direction VARCHAR(1500) NOT NULL DEFAULT '',
    created_by BIGINT UNSIGNED NOT NULL,
    updated_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    updated_at DATETIME(6) NOT NULL,
    INDEX proveedores_name (supplier_name(191), id),
    CONSTRAINT proveedores_creator_fk FOREIGN KEY (created_by) REFERENCES users(id),
    CONSTRAINT proveedores_updater_fk FOREIGN KEY (updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE proveedores_create_receipts (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY (actor_id, key_hash),
    CONSTRAINT proveedores_receipt_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE proveedores_actor_limits (
    actor_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    window_start DATETIME(6) NOT NULL,
    requests INT UNSIGNED NOT NULL,
    CONSTRAINT proveedores_limit_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
