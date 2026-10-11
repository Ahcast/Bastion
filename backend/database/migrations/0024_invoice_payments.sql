CREATE TABLE payments (
 id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
 invoice_id BIGINT UNSIGNED NOT NULL,
 payment_date DATE NOT NULL,
 method VARCHAR(16) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 currency CHAR(3) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 amount DECIMAL(18,2) NOT NULL,
 reference VARCHAR(500) NOT NULL,
 reason VARCHAR(1000) NOT NULL,
 receipt_number VARCHAR(80) NULL UNIQUE,
 version BIGINT UNSIGNED NOT NULL,
 created_by BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 voided_at DATETIME(6) NULL,
 voided_by BIGINT UNSIGNED NULL,
 void_reason VARCHAR(1000) NULL,
 FOREIGN KEY(invoice_id) REFERENCES invoices(id),
 FOREIGN KEY(created_by) REFERENCES users(id),
 FOREIGN KEY(voided_by) REFERENCES users(id),
 INDEX payment_invoice_date(invoice_id,payment_date,id),
 INDEX payment_invoice_active(invoice_id,voided_at,id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT IGNORE INTO permissions(name) VALUES('facturas.payment'),('facturas.payment.void');
