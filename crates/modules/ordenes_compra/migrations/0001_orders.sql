-- New Bastion records only; never copies Legacy data or guesses destination IDs.
CREATE TABLE ordenes_destinations (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    version BIGINT UNSIGNED NOT NULL DEFAULT 1,
    destination VARCHAR(255) NOT NULL,
    name_key VARCHAR(255) NOT NULL,
    UNIQUE KEY ordenes_destination_name (name_key)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ordenes_orders (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    version BIGINT UNSIGNED NOT NULL DEFAULT 1,
    status VARCHAR(20) NOT NULL,
    n_orders VARCHAR(50) NOT NULL,
    n_bill VARCHAR(120) NOT NULL,
    client_name VARCHAR(255) NOT NULL,
    order_date DATE NOT NULL,
    id_destination BIGINT UNSIGNED NOT NULL,
    total DECIMAL(14,2) NOT NULL,
    data_json JSON NOT NULL,
    calculation_json JSON NOT NULL,
    created_by BIGINT UNSIGNED NOT NULL,
    updated_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    updated_at DATETIME(6) NOT NULL,
    KEY ordenes_date (created_at,id),
    KEY ordenes_status (status,created_at,id),
    CONSTRAINT ordenes_destination_fk FOREIGN KEY (id_destination) REFERENCES ordenes_destinations(id),
    CONSTRAINT ordenes_creator_fk FOREIGN KEY (created_by) REFERENCES users(id),
    CONSTRAINT ordenes_updater_fk FOREIGN KEY (updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ordenes_create_receipts (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY (actor_id, key_hash),
    CONSTRAINT ordenes_receipt_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE ordenes_actor_limits (
    actor_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    window_start DATETIME(6) NOT NULL,
    requests INT UNSIGNED NOT NULL,
    CONSTRAINT ordenes_limit_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
