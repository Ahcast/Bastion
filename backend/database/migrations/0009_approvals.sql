CREATE TABLE approvals (
 id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
 action VARCHAR(80) CHARACTER SET ascii NOT NULL,
 entity_type VARCHAR(60) CHARACTER SET ascii NOT NULL,
 entity_id BIGINT UNSIGNED NOT NULL,
 entity_version BIGINT UNSIGNED NOT NULL,
 input_hash CHAR(64) CHARACTER SET ascii NOT NULL,
 owner_id BIGINT UNSIGNED NOT NULL,
 team_id VARCHAR(40) CHARACTER SET ascii NOT NULL,
 view_permission VARCHAR(100) CHARACTER SET ascii NOT NULL,
 request_permission VARCHAR(100) CHARACTER SET ascii NOT NULL,
 decision_permission VARCHAR(100) CHARACTER SET ascii NOT NULL,
 revoke_permission VARCHAR(100) CHARACTER SET ascii NOT NULL,
 requested_by BIGINT UNSIGNED NOT NULL,
 decided_by BIGINT UNSIGNED NULL,
 status VARCHAR(20) CHARACTER SET ascii NOT NULL,
 reason VARCHAR(1000) NOT NULL,
 expires_at DATETIME(6) NOT NULL,
 version BIGINT UNSIGNED NOT NULL DEFAULT 1,
 created_at DATETIME(6) NOT NULL,
 updated_at DATETIME(6) NOT NULL,
 UNIQUE(action,entity_type,entity_id,entity_version),
 INDEX approval_entity(entity_type,entity_id,id),
 FOREIGN KEY(owner_id) REFERENCES users(id),
 FOREIGN KEY(requested_by) REFERENCES users(id),
 FOREIGN KEY(decided_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE approval_events (
 approval_id BIGINT UNSIGNED NOT NULL,
 version BIGINT UNSIGNED NOT NULL,
 status VARCHAR(20) CHARACTER SET ascii NOT NULL,
 reason VARCHAR(1000) NOT NULL,
 actor_id BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 PRIMARY KEY(approval_id,version),
 FOREIGN KEY(approval_id) REFERENCES approvals(id),
 FOREIGN KEY(actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT IGNORE INTO permissions(name) VALUES('cotizaciones.request_approval'),('cotizaciones.approve'),('cotizaciones.revoke_approval');
