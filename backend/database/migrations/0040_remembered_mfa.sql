CREATE TABLE mfa_browser_verifications (
 token_hash CHAR(64) CHARACTER SET ascii PRIMARY KEY,
 user_id BIGINT UNSIGNED NOT NULL,
 ip_hash CHAR(64) CHARACTER SET ascii NOT NULL,
 created_at DATETIME(6) NOT NULL,
 expires_at DATETIME(6) NOT NULL,
 FOREIGN KEY(user_id) REFERENCES users(id),
 INDEX mfa_browser_user(user_id),
 INDEX mfa_browser_expiry(expires_at)
) ENGINE=InnoDB;
ALTER TABLE sessions
 ADD COLUMN ip_hash CHAR(64) CHARACTER SET ascii NULL,
 ADD COLUMN family_hash CHAR(64) CHARACTER SET ascii NULL,
 ADD COLUMN browser_hash CHAR(64) CHARACTER SET ascii NULL,
 ADD INDEX sessions_browser(browser_hash),
 ADD INDEX sessions_family(family_hash);
