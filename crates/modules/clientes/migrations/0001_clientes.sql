-- New Bastion catalog only; no import or mutation of Legacy clients.
CREATE TABLE clientes_records (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    version BIGINT UNSIGNED NOT NULL DEFAULT 1,
    cliente_name VARCHAR(255) NOT NULL,
    user_contact VARCHAR(255) NOT NULL DEFAULT '',
    email VARCHAR(50) NOT NULL DEFAULT '',
    rtn VARCHAR(30) NOT NULL DEFAULT '',
    phone_number VARCHAR(20) NOT NULL DEFAULT '',
    mobile_number VARCHAR(20) NOT NULL DEFAULT '',
    direction VARCHAR(1500) NOT NULL DEFAULT '',
    name_hash BINARY(32) NOT NULL,
    sort_key VARCHAR(1020) COLLATE utf8mb4_bin NOT NULL,
    rtn_digits VARCHAR(30) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    catalog_score TINYINT UNSIGNED NOT NULL,
    created_by BIGINT UNSIGNED NOT NULL,
    updated_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    updated_at DATETIME(6) NOT NULL,
    INDEX clientes_name (name_hash, catalog_score, id),
    INDEX clientes_rtn (rtn_digits, id),
    INDEX clientes_sort (sort_key(191), id),
    CONSTRAINT clientes_creator_fk FOREIGN KEY (created_by) REFERENCES users(id),
    CONSTRAINT clientes_updater_fk FOREIGN KEY (updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE clientes_create_receipts (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY (actor_id, key_hash),
    CONSTRAINT clientes_receipt_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE clientes_actor_limits (
    actor_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    window_start DATETIME(6) NOT NULL,
    requests INT UNSIGNED NOT NULL,
    CONSTRAINT clientes_limit_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
