ALTER TABLE invoices
 ADD COLUMN issued_at DATETIME(6) NULL,
 ADD COLUMN issued_by BIGINT UNSIGNED NULL,
 ADD COLUMN due_date DATE NULL,
 ADD COLUMN company_snapshot_json JSON NULL,
 ADD COLUMN company_version BIGINT UNSIGNED NULL,
 ADD FOREIGN KEY(issued_by) REFERENCES users(id),
 ADD INDEX invoice_due_date(due_date,id);
INSERT IGNORE INTO permissions(name) VALUES('facturas.emit');
