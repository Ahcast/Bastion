ALTER TABLE security_events
  ADD KEY ix_security_event_ip_type_result_date (ip_address, event_type, result, created_at);

ALTER TABLE bootstrap_setups
  ADD COLUMN failed_attempts TINYINT UNSIGNED NOT NULL DEFAULT 0;

CREATE TABLE user_smtp_credentials (
  user_id BIGINT UNSIGNED NOT NULL,
  smtp_email VARCHAR(254) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  password_ciphertext BLOB NOT NULL,
  updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (user_id),
  CONSTRAINT fk_user_smtp_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE account_tokens (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NOT NULL,
  token_hash BINARY(32) NOT NULL,
  token_type VARCHAR(32) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  target_email VARCHAR(254) CHARACTER SET ascii COLLATE ascii_general_ci NULL,
  created_by BIGINT UNSIGNED NULL,
  expires_at DATETIME(6) NULL,
  mailed_at DATETIME(6) NULL,
  consumed_at DATETIME(6) NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY uq_account_tokens_hash (token_hash),
  KEY ix_account_tokens_user_type (user_id, token_type, consumed_at, expires_at),
  CONSTRAINT fk_account_tokens_user FOREIGN KEY (user_id) REFERENCES users(id),
  CONSTRAINT fk_account_tokens_creator FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE mail_outbox (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  kind VARCHAR(48) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  sender_user_id BIGINT UNSIGNED NOT NULL,
  recipient_email VARCHAR(254) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  subject VARCHAR(180) NOT NULL,
  body_ciphertext MEDIUMBLOB NOT NULL,
  account_token_id BIGINT UNSIGNED NULL,
  delivery_status VARCHAR(24) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL DEFAULT 'pending',
  attempts SMALLINT UNSIGNED NOT NULL DEFAULT 0,
  available_at DATETIME(6) NOT NULL,
  last_error_code VARCHAR(80) CHARACTER SET ascii COLLATE ascii_general_ci NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  sent_at DATETIME(6) NULL,
  PRIMARY KEY (id),
  KEY ix_mail_outbox_due (delivery_status, available_at, id),
  CONSTRAINT fk_mail_sender FOREIGN KEY (sender_user_id) REFERENCES users(id),
  CONSTRAINT fk_mail_token FOREIGN KEY (account_token_id) REFERENCES account_tokens(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE totp_enrollment_tokens (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NOT NULL,
  token_hash BINARY(32) NOT NULL,
  secret_ciphertext BLOB NOT NULL,
  token_type VARCHAR(24) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  expires_at DATETIME(6) NOT NULL,
  consumed_at DATETIME(6) NULL,
  failed_attempts TINYINT UNSIGNED NOT NULL DEFAULT 0,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY uq_totp_enrollment_token_hash (token_hash),
  KEY ix_totp_enrollment_user (user_id, consumed_at, expires_at),
  CONSTRAINT fk_totp_enrollment_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
