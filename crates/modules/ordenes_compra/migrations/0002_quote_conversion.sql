
ALTER TABLE ordenes_orders
    ADD COLUMN source_quote_id BIGINT UNSIGNED NULL,
    ADD COLUMN source_quote_version BIGINT UNSIGNED NULL,
    ADD COLUMN source_quote_number VARCHAR(100) NULL,
    ADD UNIQUE KEY ordenes_source_quote (source_quote_id);

CREATE TABLE ordenes_conversion_receipts (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY(actor_id,key_hash),
    CONSTRAINT ordenes_conversion_actor_fk FOREIGN KEY(actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
