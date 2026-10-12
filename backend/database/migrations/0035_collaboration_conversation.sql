CREATE TABLE collaboration_comments (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
 resource_type VARCHAR(60) CHARACTER SET ascii NOT NULL,
 resource_id VARCHAR(80) CHARACTER SET ascii NOT NULL,
 author_id BIGINT UNSIGNED NOT NULL,
 body VARCHAR(4000) NOT NULL,
 version BIGINT UNSIGNED NOT NULL DEFAULT 1,
 created_at DATETIME(6) NOT NULL,
 deleted_at DATETIME(6) NULL,
 INDEX collaboration_comment_resource(resource_type,resource_id,deleted_at,created_at,id),
 FOREIGN KEY(author_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE collaboration_mentions (
 comment_id BIGINT UNSIGNED NOT NULL,
 user_id BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 PRIMARY KEY(comment_id,user_id),
 INDEX collaboration_mention_user(user_id,comment_id),
 FOREIGN KEY(comment_id) REFERENCES collaboration_comments(id),
 FOREIGN KEY(user_id) REFERENCES users(id)
) ENGINE=InnoDB;

CREATE TABLE collaboration_notifications (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
 recipient_id BIGINT UNSIGNED NOT NULL,
 comment_id BIGINT UNSIGNED NOT NULL,
 created_at DATETIME(6) NOT NULL,
 read_at DATETIME(6) NULL,
 UNIQUE KEY collaboration_notification_once(recipient_id,comment_id),
 INDEX collaboration_notification_cursor(recipient_id,id),
 FOREIGN KEY(recipient_id) REFERENCES users(id),
 FOREIGN KEY(comment_id) REFERENCES collaboration_comments(id)
) ENGINE=InnoDB;

CREATE TABLE collaboration_activity (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
 resource_type VARCHAR(60) CHARACTER SET ascii NOT NULL,
 resource_id VARCHAR(80) CHARACTER SET ascii NOT NULL,
 actor_id BIGINT UNSIGNED NOT NULL,
 action VARCHAR(40) CHARACTER SET ascii NOT NULL,
 comment_id BIGINT UNSIGNED NULL,
 created_at DATETIME(6) NOT NULL,
 INDEX collaboration_activity_resource(resource_type,resource_id,created_at,id),
 FOREIGN KEY(actor_id) REFERENCES users(id),
 FOREIGN KEY(comment_id) REFERENCES collaboration_comments(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT IGNORE INTO permissions(name) VALUES
 ('colaboracion.comment'),
 ('colaboracion.moderate');
