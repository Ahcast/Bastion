CREATE TABLE collaboration_links (
 entry_id BIGINT UNSIGNED NOT NULL,
 resource_type VARCHAR(60) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 resource_id BIGINT UNSIGNED NOT NULL,
 resource_version BIGINT UNSIGNED NOT NULL,
 label_snapshot VARCHAR(1000) NOT NULL,
 linked_by BIGINT UNSIGNED NOT NULL,
 linked_at DATETIME(6) NOT NULL,
 PRIMARY KEY(entry_id,resource_type,resource_id),
 FOREIGN KEY(entry_id) REFERENCES collaboration_entries(id),
 FOREIGN KEY(linked_by) REFERENCES users(id),
 INDEX collaboration_link_resource(resource_type,resource_id,entry_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT IGNORE INTO permissions(name) VALUES('colaboracion.link');
