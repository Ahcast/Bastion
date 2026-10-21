SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci;

CREATE TABLE persons (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  full_name VARCHAR(180) NOT NULL,
  dni VARCHAR(40) NULL,
  country_code CHAR(2) NULL,
  gender VARCHAR(32) NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY uq_persons_dni (dni)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE roles (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  system_key VARCHAR(64) NULL,
  name VARCHAR(100) NOT NULL,
  description VARCHAR(500) NULL,
  protected TINYINT(1) NOT NULL DEFAULT 0,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY uq_roles_system_key (system_key),
  UNIQUE KEY uq_roles_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE users (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  person_id BIGINT UNSIGNED NOT NULL,
  role_id BIGINT UNSIGNED NOT NULL,
  email VARCHAR(254) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  pending_email VARCHAR(254) CHARACTER SET ascii COLLATE ascii_general_ci NULL,
  password_hash VARCHAR(255) NULL,
  status VARCHAR(32) NOT NULL DEFAULT 'invitation_pending',
  created_by BIGINT UNSIGNED NULL,
  invitation_expires_at DATETIME(6) NULL,
  totp_secret_enc BLOB NULL,
  totp_enabled TINYINT(1) NOT NULL DEFAULT 0,
  last_totp_counter BIGINT NULL,
  failed_password_count INT UNSIGNED NOT NULL DEFAULT 0,
  lock_level TINYINT UNSIGNED NOT NULL DEFAULT 0,
  locked_until DATETIME(6) NULL,
  incident_alerted_at DATETIME(6) NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY uq_users_person (person_id),
  UNIQUE KEY uq_users_email (email),
  KEY ix_users_status_role (status, role_id),
  CONSTRAINT fk_users_person FOREIGN KEY (person_id) REFERENCES persons(id),
  CONSTRAINT fk_users_role FOREIGN KEY (role_id) REFERENCES roles(id),
  CONSTRAINT fk_users_creator FOREIGN KEY (created_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE role_permissions (
  role_id BIGINT UNSIGNED NOT NULL,
  module_id VARCHAR(80) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  scope_key VARCHAR(120) NOT NULL DEFAULT '*',
  capability VARCHAR(16) NOT NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  scope_hash BINARY(32) NOT NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uq_role_permission (role_id, module_id, scope_hash, capability),
  CONSTRAINT fk_role_permissions_role FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE module_registry (
  module_id VARCHAR(80) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  display_name VARCHAR(120) NOT NULL,
  included_in_release TINYINT(1) NOT NULL DEFAULT 0,
  environment_allowed TINYINT(1) NOT NULL DEFAULT 0,
  enabled TINYINT(1) NOT NULL DEFAULT 0,
  settings_schema_version INT UNSIGNED NOT NULL DEFAULT 1,
  updated_by BIGINT UNSIGNED NULL,
  updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (module_id),
  CONSTRAINT fk_modules_updated_by FOREIGN KEY (updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE settings (
  module_id VARCHAR(80) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  scope_key VARCHAR(120) NOT NULL,
  setting_key VARCHAR(120) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  schema_version INT UNSIGNED NOT NULL,
  value_json JSON NOT NULL,
  secret_ciphertext BLOB NULL,
  updated_by BIGINT UNSIGNED NULL,
  updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  lookup_hash BINARY(32) NOT NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uq_settings_lookup_hash (lookup_hash),
  CONSTRAINT fk_settings_module FOREIGN KEY (module_id) REFERENCES module_registry(module_id),
  CONSTRAINT fk_settings_updated_by FOREIGN KEY (updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE sessions (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NOT NULL,
  token_hash BINARY(32) NOT NULL,
  csrf_hash BINARY(32) NOT NULL,
  user_agent VARCHAR(512) NOT NULL DEFAULT '',
  ip_address VARCHAR(45) NOT NULL,
  keep_logged_in TINYINT(1) NOT NULL DEFAULT 0,
  started_at DATETIME(6) NOT NULL,
  last_activity_at DATETIME(6) NOT NULL,
  last_totp_at DATETIME(6) NOT NULL,
  expires_at DATETIME(6) NULL,
  revoked_at DATETIME(6) NULL,
  revocation_reason VARCHAR(48) NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uq_sessions_token_hash (token_hash),
  KEY ix_sessions_user_active (user_id, revoked_at, expires_at),
  CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE trusted_devices (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NOT NULL,
  token_hash BINARY(32) NOT NULL,
  device_label VARCHAR(180) NOT NULL DEFAULT 'Dispositivo desconocido',
  ip_address VARCHAR(45) NOT NULL,
  created_at DATETIME(6) NOT NULL,
  last_used_at DATETIME(6) NOT NULL,
  last_totp_at DATETIME(6) NOT NULL,
  expires_at DATETIME(6) NOT NULL,
  revoked_at DATETIME(6) NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uq_trusted_device_hash (token_hash),
  KEY ix_trusted_device_user (user_id, revoked_at, expires_at),
  CONSTRAINT fk_trusted_device_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE otp_challenges (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NOT NULL,
  challenge_hash BINARY(32) NOT NULL,
  client_ip VARCHAR(45) NOT NULL,
  user_agent VARCHAR(512) NOT NULL DEFAULT '',
  keep_logged_in TINYINT(1) NOT NULL DEFAULT 0,
  attempts TINYINT UNSIGNED NOT NULL DEFAULT 0,
  expires_at DATETIME(6) NOT NULL,
  consumed_at DATETIME(6) NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uq_otp_challenge_hash (challenge_hash),
  KEY ix_otp_user_ip (user_id, client_ip, expires_at),
  CONSTRAINT fk_otp_challenge_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE totp_recovery_codes (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NOT NULL,
  code_hash BINARY(32) NOT NULL,
  consumed_at DATETIME(6) NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (id),
  UNIQUE KEY uq_recovery_code_hash (code_hash),
  KEY ix_recovery_user (user_id, consumed_at),
  CONSTRAINT fk_recovery_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE bootstrap_setups (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NOT NULL,
  token_hash BINARY(32) NOT NULL,
  expires_at DATETIME(6) NOT NULL,
  consumed_at DATETIME(6) NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uq_bootstrap_token_hash (token_hash),
  CONSTRAINT fk_bootstrap_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE core_bootstrap_state (
  id TINYINT UNSIGNED NOT NULL,
  pending_user_id BIGINT UNSIGNED NULL,
  pending_expires_at DATETIME(6) NULL,
  completed_at DATETIME(6) NULL,
  PRIMARY KEY (id),
  CONSTRAINT fk_bootstrap_state_user FOREIGN KEY (pending_user_id) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT INTO core_bootstrap_state (id) VALUES (1);

CREATE TABLE security_events (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id BIGINT UNSIGNED NULL,
  event_type VARCHAR(64) NOT NULL,
  result VARCHAR(24) NOT NULL,
  ip_address VARCHAR(45) NOT NULL,
  device_label VARCHAR(180) NOT NULL DEFAULT '',
  created_at DATETIME(6) NOT NULL,
  PRIMARY KEY (id),
  KEY ix_security_event_user_date (user_id, created_at),
  CONSTRAINT fk_security_event_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE audit_events (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  actor_id BIGINT UNSIGNED NULL,
  module_id VARCHAR(80) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  action VARCHAR(120) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  target_type VARCHAR(80) NOT NULL,
  target_id VARCHAR(120) NULL,
  result VARCHAR(24) NOT NULL,
  details_json JSON NULL,
  created_at DATETIME(6) NOT NULL,
  PRIMARY KEY (id),
  KEY ix_audit_date_id (created_at, id),
  KEY ix_audit_actor_date (actor_id, created_at),
  KEY ix_audit_module_action (module_id, action, created_at),
  CONSTRAINT fk_audit_actor FOREIGN KEY (actor_id) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE admin_notifications (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  recipient_user_id BIGINT UNSIGNED NULL,
  kind VARCHAR(64) NOT NULL,
  subject VARCHAR(180) NOT NULL,
  payload_json JSON NOT NULL,
  delivery_status VARCHAR(24) NOT NULL DEFAULT 'pending',
  last_error_code VARCHAR(80) NULL,
  created_at DATETIME(6) NOT NULL,
  sent_at DATETIME(6) NULL,
  PRIMARY KEY (id),
  KEY ix_admin_notifications_status_date (delivery_status, created_at),
  CONSTRAINT fk_notification_recipient FOREIGN KEY (recipient_user_id) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT INTO roles (system_key, name, description, protected)
VALUES ('system_administrator', 'Administrador del sistema', 'Rol integrado con acceso completo.', 1);

INSERT INTO module_registry (module_id, display_name, included_in_release, environment_allowed, enabled)
VALUES ('core', 'Core', 1, 1, 1);
