CREATE TABLE ai_tasks (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
 task_type VARCHAR(32) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 process_id BIGINT UNSIGNED NOT NULL,
 input_version BIGINT UNSIGNED NOT NULL,
 input_hash CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 input_snapshot MEDIUMTEXT NOT NULL,
 skill_version VARCHAR(100) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 requested_by BIGINT UNSIGNED NOT NULL,
 status VARCHAR(24) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 version BIGINT UNSIGNED NOT NULL DEFAULT 1,
 attempts INT UNSIGNED NOT NULL DEFAULT 0,
 available_at DATETIME(6) NOT NULL,
 current_run_id BIGINT UNSIGNED NULL,
 created_at DATETIME(6) NOT NULL,
 updated_at DATETIME(6) NOT NULL,
 UNIQUE(task_type,process_id),
 FOREIGN KEY(process_id,input_version) REFERENCES process_versions(process_id,version),
 FOREIGN KEY(requested_by) REFERENCES users(id),
 INDEX ai_pending(status,available_at,process_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE ai_runs (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
 task_id BIGINT UNSIGNED NOT NULL,
 attempt INT UNSIGNED NOT NULL,
 actor_key VARCHAR(100) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 actor_user_id BIGINT UNSIGNED NOT NULL,
 skill_version VARCHAR(100) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 status VARCHAR(24) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 lease_hash CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NULL,
 lease_cipher TEXT NULL,
 lease_expires_at DATETIME(6) NOT NULL,
 renewals INT UNSIGNED NOT NULL DEFAULT 0,
 started_at DATETIME(6) NOT NULL,
 finished_at DATETIME(6) NULL,
 error_code VARCHAR(32) CHARACTER SET ascii NULL,
 UNIQUE(task_id,attempt),
 UNIQUE(task_id,id),
 FOREIGN KEY(task_id) REFERENCES ai_tasks(id),
 FOREIGN KEY(actor_user_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
ALTER TABLE ai_tasks ADD CONSTRAINT ai_current_run FOREIGN KEY(id,current_run_id) REFERENCES ai_runs(task_id,id);
INSERT IGNORE INTO permissions(name) VALUES('ai.tasks.read'),('ai.tasks.create'),('ai.tasks.execute'),('ai.tasks.cancel');
