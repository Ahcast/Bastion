ALTER TABLE customer_purchase_order_items ADD UNIQUE KEY customer_po_item_identity(purchase_order_id,id);
CREATE TABLE customer_purchase_order_documents (
 document_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
 purchase_order_id BIGINT UNSIGNED NOT NULL,
 item_id BIGINT UNSIGNED NULL,
 created_by BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 FOREIGN KEY(document_id) REFERENCES documents(id),
 FOREIGN KEY(purchase_order_id) REFERENCES customer_purchase_orders(id),
 FOREIGN KEY(purchase_order_id,item_id) REFERENCES customer_purchase_order_items(purchase_order_id,id),
 FOREIGN KEY(created_by) REFERENCES users(id),
 INDEX customer_po_document_target(purchase_order_id,item_id,document_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT IGNORE INTO permissions(name) VALUES ('ordenes.documents.write');
