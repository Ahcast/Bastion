CREATE TABLE customer_purchase_order_clones (
 purchase_order_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
 source_purchase_order_id BIGINT UNSIGNED NOT NULL,
 source_version BIGINT UNSIGNED NOT NULL,
 created_by BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 FOREIGN KEY(purchase_order_id) REFERENCES customer_purchase_orders(id),
 FOREIGN KEY(source_purchase_order_id) REFERENCES customer_purchase_orders(id),
 FOREIGN KEY(created_by) REFERENCES users(id),
 INDEX customer_po_clone_source(source_purchase_order_id,purchase_order_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
