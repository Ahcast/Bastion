CREATE TABLE oauth_perimeter_quotas (
  client_hash BINARY(32) NOT NULL,
  window_start DATETIME(6) NOT NULL,
  requests INT UNSIGNED NOT NULL,
  PRIMARY KEY (client_hash),
  KEY ix_oauth_perimeter_expiry (window_start)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
