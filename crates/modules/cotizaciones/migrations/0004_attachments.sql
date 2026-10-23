ALTER TABLE cotizaciones_documents
    MODIFY filename VARCHAR(255) NOT NULL,
    ADD attachment_id CHAR(48) CHARACTER SET ascii COLLATE ascii_bin NULL,
    ADD title VARCHAR(255) NOT NULL DEFAULT '',
    ADD UNIQUE KEY cotizaciones_attachment_id (attachment_id);

CREATE TABLE cotizaciones_attachment_receipts (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    quote_id BIGINT UNSIGNED NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY (actor_id,key_hash),
    CONSTRAINT cotizaciones_attachment_receipt_quote_fk FOREIGN KEY (quote_id) REFERENCES cotizaciones_drafts(id) ON DELETE CASCADE,
    CONSTRAINT cotizaciones_attachment_receipt_actor_fk FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
