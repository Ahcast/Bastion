ALTER TABLE invoices
 ADD COLUMN cancelled_at DATETIME(6) NULL,
 ADD COLUMN cancelled_by BIGINT UNSIGNED NULL,
 ADD COLUMN cancel_reason VARCHAR(1000) NULL,
 ADD COLUMN cancellation_attested BOOLEAN NOT NULL DEFAULT FALSE,
 ADD FOREIGN KEY (cancelled_by) REFERENCES users(id);

INSERT IGNORE INTO permissions(name) VALUES('facturas.cancel');
