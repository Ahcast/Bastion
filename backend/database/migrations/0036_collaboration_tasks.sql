CREATE TABLE collaboration_tasks (
 id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
 creator_id BIGINT UNSIGNED NOT NULL,
 assignee_id BIGINT UNSIGNED NOT NULL,
 resource_type VARCHAR(60) CHARACTER SET ascii COLLATE ascii_bin NULL,
 resource_id BIGINT UNSIGNED NULL,
 title VARCHAR(180) NOT NULL,
 note VARCHAR(4000) NOT NULL,
 due_date DATE NULL,
 status VARCHAR(20) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 version BIGINT UNSIGNED NOT NULL DEFAULT 1,
 created_at DATETIME(6) NOT NULL,
 updated_at DATETIME(6) NOT NULL,
 FOREIGN KEY(creator_id) REFERENCES users(id),
 FOREIGN KEY(assignee_id) REFERENCES users(id),
 INDEX collaboration_task_assignee(assignee_id,status,due_date,id),
 INDEX collaboration_task_creator(creator_id,status,id),
 INDEX collaboration_task_resource(resource_type,resource_id,id),
 CONSTRAINT collaboration_task_resource_pair CHECK ((resource_type IS NULL AND resource_id IS NULL) OR (resource_type IS NOT NULL AND resource_id IS NOT NULL))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT IGNORE INTO permissions(name) VALUES('colaboracion.task');
