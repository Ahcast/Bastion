-- Keep candidate redirect URIs separate from the currently approved client
-- until an administrator reviews the exact metadata document.
CREATE TABLE oauth_client_candidate_redirects (
  client_id BIGINT UNSIGNED NOT NULL,
  metadata_hash BINARY(32) NOT NULL,
  uri_hash BINARY(32) NOT NULL,
  uri TEXT NOT NULL,
  PRIMARY KEY (client_id, metadata_hash, uri_hash),
  CONSTRAINT fk_oauth_candidate_client FOREIGN KEY (client_id) REFERENCES oauth_clients(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
