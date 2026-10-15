ALTER TABLE mfa_browser_verifications
 ADD COLUMN device_id CHAR(32) CHARACTER SET ascii NULL,
 ADD COLUMN device_name VARCHAR(80) NOT NULL DEFAULT 'Navegador reconocido',
 ADD COLUMN fingerprint_hash CHAR(64) CHARACTER SET ascii NULL,
 ADD COLUMN last_seen_at DATETIME(6) NULL;
UPDATE mfa_browser_verifications SET device_id=LEFT(SHA2(CONCAT('device:',token_hash),256),32),last_seen_at=created_at;
ALTER TABLE mfa_browser_verifications
 MODIFY COLUMN device_id CHAR(32) CHARACTER SET ascii NOT NULL,
 MODIFY COLUMN last_seen_at DATETIME(6) NOT NULL,
 ADD UNIQUE INDEX mfa_browser_device(device_id);
