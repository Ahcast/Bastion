ALTER TABLE invoices
 ADD source_purchase_order_id BIGINT UNSIGNED NULL,
 ADD source_purchase_order_version BIGINT UNSIGNED NULL,
 ADD UNIQUE KEY invoice_source_purchase_order(source_purchase_order_id),
 ADD FOREIGN KEY(source_purchase_order_id) REFERENCES customer_purchase_orders(id);
ALTER TABLE customer_purchase_orders
 ADD invoice_id BIGINT UNSIGNED NULL,
 ADD invoice_number_snapshot VARCHAR(80) NULL,
 ADD UNIQUE KEY customer_order_invoice(invoice_id),
 ADD FOREIGN KEY(invoice_id) REFERENCES invoices(id);
INSERT IGNORE INTO permissions(name) VALUES('ordenes.convert'),('ordenes.close'),('ordenes.reopen');
