ALTER TABLE customer_purchase_orders
 ADD source_quote_version BIGINT UNSIGNED NULL,
 ADD UNIQUE KEY customer_po_unique_quote(source_quote_id),
 ADD CONSTRAINT customer_po_quote_version FOREIGN KEY(source_quote_id,source_quote_version) REFERENCES quote_versions(quote_id,version);

ALTER TABLE quotes
 ADD purchase_order_id BIGINT UNSIGNED NULL,
 ADD UNIQUE KEY quote_unique_purchase_order(purchase_order_id),
 ADD CONSTRAINT quote_purchase_order FOREIGN KEY(purchase_order_id) REFERENCES customer_purchase_orders(id);

INSERT IGNORE INTO permissions(name) VALUES ('cotizaciones.convert');
