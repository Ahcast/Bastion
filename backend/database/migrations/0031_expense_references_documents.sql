CREATE TABLE expense_references (
 expense_id BIGINT UNSIGNED NOT NULL,
 entity_type VARCHAR(32) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 entity_id BIGINT UNSIGNED NOT NULL,
 entity_version BIGINT UNSIGNED NOT NULL,
 label_snapshot VARCHAR(1000) NOT NULL,
 linked_by BIGINT UNSIGNED NOT NULL,
 linked_at DATETIME(6) NOT NULL,
 PRIMARY KEY(expense_id,entity_type),
 FOREIGN KEY(expense_id) REFERENCES expenses(id),
 FOREIGN KEY(linked_by) REFERENCES users(id),
 INDEX expense_reference_entity(entity_type,entity_id),
 INDEX expense_reference_actor(linked_by,linked_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE expense_documents (
 expense_id BIGINT UNSIGNED NOT NULL,
 document_id BIGINT UNSIGNED NOT NULL,
 document_version BIGINT UNSIGNED NOT NULL,
 linked_by BIGINT UNSIGNED NOT NULL,
 linked_at DATETIME(6) NOT NULL,
 PRIMARY KEY(expense_id,document_id),
 UNIQUE KEY expense_document_unique(document_id),
 FOREIGN KEY(expense_id) REFERENCES expenses(id),
 FOREIGN KEY(document_id) REFERENCES documents(id),
 FOREIGN KEY(linked_by) REFERENCES users(id),
 INDEX expense_document_actor(linked_by,linked_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT IGNORE INTO permissions(name) VALUES('gastos.references.write'),('gastos.documents.write');
