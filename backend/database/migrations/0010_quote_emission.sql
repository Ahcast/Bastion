ALTER TABLE quote_pdf_snapshots
 ADD COLUMN kind VARCHAR(20) CHARACTER SET ascii NOT NULL DEFAULT 'draft',
 ADD COLUMN approval_id BIGINT UNSIGNED NULL,
 ADD COLUMN binding_hash CHAR(64) CHARACTER SET ascii NULL,
 ADD COLUMN issued_by BIGINT UNSIGNED NULL,
 ADD COLUMN issued_at DATETIME(6) NULL,
 ADD UNIQUE(approval_id),
 ADD FOREIGN KEY(approval_id) REFERENCES approvals(id),
 ADD FOREIGN KEY(issued_by) REFERENCES users(id);
INSERT IGNORE INTO permissions(name) VALUES('cotizaciones.emit');
