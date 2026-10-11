CREATE TABLE customer_purchase_order_status_history (
 id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
 purchase_order_id BIGINT UNSIGNED NOT NULL,
 item_position SMALLINT UNSIGNED NULL,
 from_status VARCHAR(32) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 to_status VARCHAR(32) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 actor_id BIGINT UNSIGNED NOT NULL,
 purchase_order_version BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 FOREIGN KEY(purchase_order_id) REFERENCES customer_purchase_orders(id),
 FOREIGN KEY(actor_id) REFERENCES users(id),
 INDEX customer_po_status_history_order(purchase_order_id,id),
 INDEX customer_po_status_history_item(purchase_order_id,item_position,id),
 INDEX customer_po_status_history_actor(actor_id,id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT IGNORE INTO permissions(name) VALUES
 ('ordenes.transition'),
 ('ordenes.items.transition'),
 ('ordenes.deliver');
