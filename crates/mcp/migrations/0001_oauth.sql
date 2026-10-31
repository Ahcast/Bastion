-- Staged only: not wired into the server migrator until OAuth is complete.
-- All times are UTC. Secrets are stored only as purpose-separated digests.
CREATE TABLE oauth_clients (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  client_id TEXT NOT NULL,
  client_id_hash BINARY(32) NOT NULL,
  display_name VARCHAR(255) NOT NULL,
  metadata_hash BINARY(32) NOT NULL,
  approved_metadata_hash BINARY(32) NULL,
  enabled TINYINT(1) NOT NULL DEFAULT 0,
  approved_by BIGINT UNSIGNED NULL,
  approved_at DATETIME(6) NULL,
  created_at DATETIME(6) NOT NULL,
  updated_at DATETIME(6) NOT NULL,
  UNIQUE KEY uq_oauth_client_identity (client_id_hash),
  CONSTRAINT fk_oauth_client_approver FOREIGN KEY (approved_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE oauth_client_redirects (
  client_id BIGINT UNSIGNED NOT NULL,
  uri TEXT NOT NULL,
  uri_hash BINARY(32) NOT NULL,
  PRIMARY KEY (client_id, uri_hash),
  CONSTRAINT fk_oauth_redirect_client FOREIGN KEY (client_id) REFERENCES oauth_clients(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE oauth_client_scopes (
  client_id BIGINT UNSIGNED NOT NULL,
  scope_name VARCHAR(191) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  PRIMARY KEY (client_id, scope_name),
  CONSTRAINT fk_oauth_scope_client FOREIGN KEY (client_id) REFERENCES oauth_clients(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE oauth_client_requests (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT UNSIGNED NOT NULL,
  client_id BIGINT UNSIGNED NOT NULL,
  requested_scopes JSON NOT NULL,
  status VARCHAR(16) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  reviewed_by BIGINT UNSIGNED NULL,
  reviewed_at DATETIME(6) NULL,
  created_at DATETIME(6) NOT NULL,
  updated_at DATETIME(6) NOT NULL,
  KEY ix_oauth_request_user_status (user_id, status),
  KEY ix_oauth_request_client_status (client_id, status),
  CONSTRAINT fk_oauth_request_user FOREIGN KEY (user_id) REFERENCES users(id),
  CONSTRAINT fk_oauth_request_client FOREIGN KEY (client_id) REFERENCES oauth_clients(id),
  CONSTRAINT fk_oauth_request_reviewer FOREIGN KEY (reviewed_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE oauth_grants (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT UNSIGNED NOT NULL,
  client_id BIGINT UNSIGNED NOT NULL,
  resource_uri TEXT NOT NULL,
  scopes_json JSON NOT NULL,
  issued_at DATETIME(6) NOT NULL,
  expires_at DATETIME(6) NOT NULL,
  revoked_at DATETIME(6) NULL,
  revocation_reason VARCHAR(48) CHARACTER SET ascii COLLATE ascii_bin NULL,
  KEY ix_oauth_grants_user_active (user_id, revoked_at, expires_at),
  KEY ix_oauth_grants_client_active (client_id, revoked_at, expires_at),
  CONSTRAINT fk_oauth_grant_user FOREIGN KEY (user_id) REFERENCES users(id),
  CONSTRAINT fk_oauth_grant_client FOREIGN KEY (client_id) REFERENCES oauth_clients(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- A consent request is tied to the authenticated session and consumed once.
-- The state value is opaque client data; it must never be written to logs.
CREATE TABLE oauth_consent_requests (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  request_hash BINARY(32) NOT NULL,
  user_id BIGINT UNSIGNED NOT NULL,
  session_id BIGINT UNSIGNED NOT NULL,
  client_id BIGINT UNSIGNED NOT NULL,
  redirect_uri TEXT NOT NULL,
  resource_uri TEXT NOT NULL,
  scopes_json JSON NOT NULL,
  challenge CHAR(43) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  state_value TEXT NULL,
  issued_at DATETIME(6) NOT NULL,
  expires_at DATETIME(6) NOT NULL,
  consumed_at DATETIME(6) NULL,
  UNIQUE KEY uq_oauth_consent_request_hash (request_hash),
  KEY ix_oauth_consent_user_session (user_id, session_id, expires_at),
  CONSTRAINT fk_oauth_consent_user FOREIGN KEY (user_id) REFERENCES users(id),
  CONSTRAINT fk_oauth_consent_session FOREIGN KEY (session_id) REFERENCES sessions(id),
  CONSTRAINT fk_oauth_consent_client FOREIGN KEY (client_id) REFERENCES oauth_clients(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE oauth_authorization_codes (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  grant_id BIGINT UNSIGNED NOT NULL,
  code_hash BINARY(32) NOT NULL,
  redirect_uri TEXT NOT NULL,
  challenge CHAR(43) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  issued_at DATETIME(6) NOT NULL,
  expires_at DATETIME(6) NOT NULL,
  consumed_at DATETIME(6) NULL,
  UNIQUE KEY uq_oauth_code_hash (code_hash),
  KEY ix_oauth_code_grant (grant_id),
  CONSTRAINT fk_oauth_code_grant FOREIGN KEY (grant_id) REFERENCES oauth_grants(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE oauth_refresh_tokens (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  grant_id BIGINT UNSIGNED NOT NULL,
  token_hash BINARY(32) NOT NULL,
  issued_at DATETIME(6) NOT NULL,
  expires_at DATETIME(6) NOT NULL,
  consumed_at DATETIME(6) NULL,
  UNIQUE KEY uq_oauth_refresh_hash (token_hash),
  KEY ix_oauth_refresh_grant (grant_id),
  CONSTRAINT fk_oauth_refresh_grant FOREIGN KEY (grant_id) REFERENCES oauth_grants(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE oauth_access_tokens (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  grant_id BIGINT UNSIGNED NOT NULL,
  token_hash BINARY(32) NOT NULL,
  scopes_json JSON NOT NULL,
  issued_at DATETIME(6) NOT NULL,
  expires_at DATETIME(6) NOT NULL,
  UNIQUE KEY uq_oauth_access_hash (token_hash),
  KEY ix_oauth_access_grant (grant_id),
  CONSTRAINT fk_oauth_access_grant FOREIGN KEY (grant_id) REFERENCES oauth_grants(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
