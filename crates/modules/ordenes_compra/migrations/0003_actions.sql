CREATE TABLE ordenes_clone_receipts (
 actor_id BIGINT UNSIGNED NOT NULL, key_hash BINARY(32) NOT NULL,
 request_hash BINARY(32) NOT NULL, response_json JSON NOT NULL,
 PRIMARY KEY(actor_id,key_hash), FOREIGN KEY(actor_id) REFERENCES users(id)
);
CREATE TABLE ordenes_documents (
 attachment_id CHAR(48) CHARACTER SET ascii COLLATE ascii_bin PRIMARY KEY,
 order_id BIGINT UNSIGNED NOT NULL, line_id CHAR(48) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 object_key VARCHAR(512) CHARACTER SET ascii COLLATE ascii_bin NOT NULL UNIQUE,
 filename VARCHAR(255) NOT NULL, mime VARCHAR(64) NOT NULL,
 sha256 BINARY(32) NOT NULL, byte_size INT UNSIGNED NOT NULL, created_at DATETIME(6) NOT NULL,
 UNIQUE KEY order_product(order_id,line_id), FOREIGN KEY(order_id) REFERENCES ordenes_orders(id)
);
CREATE TABLE ordenes_document_deletions (
 object_key VARCHAR(512) CHARACTER SET ascii COLLATE ascii_bin PRIMARY KEY,
 available_at DATETIME(6) NOT NULL
);
CREATE TABLE ordenes_document_receipts (
 actor_id BIGINT UNSIGNED NOT NULL, key_hash BINARY(32) NOT NULL,
 request_hash BINARY(32) NOT NULL, response_json JSON NOT NULL,
 PRIMARY KEY(actor_id,key_hash), FOREIGN KEY(actor_id) REFERENCES users(id)
);
