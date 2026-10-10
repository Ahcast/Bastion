CREATE TABLE quote_fx_observations (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  base_currency CHAR(3) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  quote_currency CHAR(3) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  rate_kind VARCHAR(16) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  rate DECIMAL(24,12) NOT NULL,
  valid_from DATETIME(6) NOT NULL,
  valid_to DATETIME(6) NULL,
  source VARCHAR(16) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  source_reference VARCHAR(1000) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL,
  source_published_at DATETIME(6) NULL,
  evidence_hash CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL DEFAULT '',
  supersedes_id BIGINT UNSIGNED NULL,
  reason VARCHAR(1000) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NOT NULL DEFAULT '',
  created_by BIGINT UNSIGNED NOT NULL,
  created_at DATETIME(6) NOT NULL,
  PRIMARY KEY (id),
  UNIQUE KEY quote_fx_single_superseder (supersedes_id),
  KEY quote_fx_lookup (base_currency,quote_currency,rate_kind,valid_from,id),
  KEY quote_fx_source (source,valid_from,id),
  CONSTRAINT quote_fx_supersedes_fk FOREIGN KEY (supersedes_id) REFERENCES quote_fx_observations(id),
  CONSTRAINT quote_fx_created_by_fk FOREIGN KEY (created_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT IGNORE INTO permissions(name) VALUES('cotizaciones.fx.view'),('cotizaciones.fx.manage');
