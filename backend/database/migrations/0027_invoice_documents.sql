CREATE TABLE invoice_document_snapshots (
 id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
 invoice_id BIGINT UNSIGNED NOT NULL,
 invoice_version BIGINT UNSIGNED NOT NULL,
 payment_id BIGINT UNSIGNED NULL,
 payment_version BIGINT UNSIGNED NULL,
 kind VARCHAR(20) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 input_hash CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 public_data MEDIUMTEXT NOT NULL,
 object_key VARCHAR(240) CHARACTER SET ascii COLLATE ascii_bin NOT NULL UNIQUE,
 document_id BIGINT UNSIGNED NULL,
 status VARCHAR(20) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 created_by BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 available_at DATETIME(6) NULL,
 UNIQUE KEY invoice_document_public(invoice_id,kind,input_hash),
 FOREIGN KEY(invoice_id) REFERENCES invoices(id),
 FOREIGN KEY(payment_id) REFERENCES payments(id),
 FOREIGN KEY(document_id) REFERENCES documents(id),
 FOREIGN KEY(created_by) REFERENCES users(id),
 INDEX invoice_document_list(invoice_id,id),
 INDEX invoice_document_payment(invoice_id,payment_id,id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT IGNORE INTO permissions(name) VALUES('facturas.documents.generate');
