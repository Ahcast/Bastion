-- Fixed-window counters, keyed by trusted actor and tool identifiers.
CREATE TABLE oauth_actor_quotas (
  actor_kind TINYINT UNSIGNED NOT NULL,
  actor_id BIGINT UNSIGNED NOT NULL,
  bucket_hash BINARY(32) NOT NULL,
  window_start DATETIME(6) NOT NULL,
  requests INT UNSIGNED NOT NULL,
  PRIMARY KEY (actor_kind, actor_id, bucket_hash)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
