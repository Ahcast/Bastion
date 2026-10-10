ALTER TABLE documents ADD COLUMN deleted_at DATETIME(6) NULL, ADD COLUMN is_generated BOOLEAN NOT NULL DEFAULT FALSE;
CREATE TABLE quote_pdf_snapshots (
 id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
 quote_id BIGINT UNSIGNED NOT NULL,
 quote_version BIGINT UNSIGNED NOT NULL,
 input_hash CHAR(64) CHARACTER SET ascii NOT NULL,
 public_data MEDIUMTEXT NOT NULL,
 object_key VARCHAR(200) CHARACTER SET ascii NOT NULL UNIQUE,
 document_id BIGINT UNSIGNED NULL,
 status VARCHAR(20) CHARACTER SET ascii NOT NULL,
 created_by BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 UNIQUE(quote_id,input_hash),
 FOREIGN KEY(quote_id,quote_version) REFERENCES quote_versions(quote_id,version),
 FOREIGN KEY(document_id) REFERENCES documents(id),
 FOREIGN KEY(created_by) REFERENCES users(id)
) ENGINE=InnoDB;
INSERT IGNORE INTO permissions(name) VALUES('cotizaciones.pdf');
