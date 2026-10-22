CREATE TABLE cotizaciones_documents (
    object_key VARCHAR(250) NOT NULL PRIMARY KEY,
    quote_id BIGINT UNSIGNED NOT NULL,
    quote_version BIGINT UNSIGNED NOT NULL,
    kind VARCHAR(20) NOT NULL,
    filename VARCHAR(180) NOT NULL,
    mime VARCHAR(100) NOT NULL,
    sha256 BINARY(32) NOT NULL,
    byte_size INT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL,
    KEY cotizaciones_documents_quote (quote_id),
    CONSTRAINT cotizaciones_document_quote_fk FOREIGN KEY (quote_id) REFERENCES cotizaciones_drafts(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE cotizaciones_document_deletions (
    object_key VARCHAR(250) NOT NULL PRIMARY KEY,
    attempts INT UNSIGNED NOT NULL DEFAULT 0,
    available_at DATETIME(6) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE cotizaciones_email_jobs (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    quote_id BIGINT UNSIGNED NULL,
    quote_version BIGINT UNSIGNED NOT NULL,
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    payload_ciphertext BLOB NOT NULL,
    delivery_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    scheduled_at DATETIME(6) NOT NULL,
    claimed_at DATETIME(6) NULL,
    completed_at DATETIME(6) NULL,
    last_error_code VARCHAR(80) NULL,
    created_at DATETIME(6) NOT NULL,
    UNIQUE KEY cotizaciones_email_idempotency (actor_id,key_hash),
    KEY cotizaciones_email_due (delivery_status,scheduled_at),
    CONSTRAINT cotizaciones_email_quote_fk FOREIGN KEY (quote_id) REFERENCES cotizaciones_drafts(id) ON DELETE SET NULL,
    CONSTRAINT cotizaciones_email_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
