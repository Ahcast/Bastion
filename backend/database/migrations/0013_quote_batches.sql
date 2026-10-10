CREATE TABLE quote_batches (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  action VARCHAR(16) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  requested_by BIGINT UNSIGNED NOT NULL,
  target_status VARCHAR(32) CHARACTER SET ascii COLLATE ascii_bin NOT NULL DEFAULT '',
  status VARCHAR(16) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  created_at DATETIME(6) NOT NULL,
  completed_at DATETIME(6) NULL,
  PRIMARY KEY (id),
  KEY quote_batches_requester (requested_by, id),
  CONSTRAINT quote_batches_requester_fk FOREIGN KEY (requested_by) REFERENCES users(id)
) ENGINE=InnoDB;

CREATE TABLE quote_batch_items (
  batch_id BIGINT UNSIGNED NOT NULL,
  position SMALLINT UNSIGNED NOT NULL,
  quote_id BIGINT UNSIGNED NOT NULL,
  quote_version BIGINT UNSIGNED NOT NULL,
  item_status VARCHAR(16) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  error_code VARCHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL DEFAULT '',
  result_json LONGTEXT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NULL,
  updated_at DATETIME(6) NOT NULL,
  PRIMARY KEY (batch_id, position),
  UNIQUE KEY quote_batch_unique_quote (batch_id, quote_id),
  KEY quote_batch_items_quote (quote_id, batch_id),
  CONSTRAINT quote_batch_items_batch_fk FOREIGN KEY (batch_id) REFERENCES quote_batches(id) ON DELETE CASCADE
) ENGINE=InnoDB;
