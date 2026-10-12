CREATE TABLE invoice_clones (
 invoice_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
 source_invoice_id BIGINT UNSIGNED NOT NULL,
 source_version BIGINT UNSIGNED NOT NULL,
 created_by BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 FOREIGN KEY(invoice_id) REFERENCES invoices(id),
 FOREIGN KEY(source_invoice_id) REFERENCES invoices(id),
 FOREIGN KEY(created_by) REFERENCES users(id),
 INDEX invoice_clone_source(source_invoice_id,invoice_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
