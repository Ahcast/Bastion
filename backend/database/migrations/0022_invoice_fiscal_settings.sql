CREATE TABLE invoice_fiscal_settings (
 id TINYINT UNSIGNED NOT NULL PRIMARY KEY,
 value JSON NOT NULL,
 version BIGINT UNSIGNED NOT NULL,
 updated_by BIGINT UNSIGNED NOT NULL,
 updated_at DATETIME(6) NOT NULL,
 FOREIGN KEY(updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT IGNORE INTO permissions(name) VALUES('facturas.fiscal.read'),('facturas.fiscal.manage');
