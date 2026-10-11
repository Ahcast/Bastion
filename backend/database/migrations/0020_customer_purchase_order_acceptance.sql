ALTER TABLE customer_purchase_orders
 ADD COLUMN deleted_at DATETIME(6) NULL,
 ADD COLUMN profit DECIMAL(18,2) NOT NULL DEFAULT 0,
 ADD COLUMN profit_version SMALLINT UNSIGNED NOT NULL DEFAULT 1,
 ADD INDEX customer_po_deleted(deleted_at,order_date,id);
UPDATE customer_purchase_orders SET profit=base_15+base_18+exempt+exonerated-investment_total;
INSERT IGNORE INTO permissions(name) VALUES('ordenes.delete');
