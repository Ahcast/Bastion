CREATE TABLE mcp_quote_email_confirmations (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    handle_hash BINARY(32) NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    grant_id BIGINT UNSIGNED NOT NULL,
    quote_id BIGINT UNSIGNED NOT NULL,
    quote_version BIGINT UNSIGNED NOT NULL,
    payload_ciphertext MEDIUMBLOB NOT NULL,
    created_at DATETIME(6) NOT NULL,
    expires_at DATETIME(6) NOT NULL,
    confirmed_at DATETIME(6) NULL,
    denied_at DATETIME(6) NULL,
    job_id BIGINT UNSIGNED NULL,
    UNIQUE KEY uq_mcp_quote_mail_handle (handle_hash),
    KEY ix_mcp_quote_mail_expiry (expires_at),
    CONSTRAINT fk_mcp_quote_mail_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_mcp_quote_mail_grant FOREIGN KEY (grant_id) REFERENCES oauth_grants(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
