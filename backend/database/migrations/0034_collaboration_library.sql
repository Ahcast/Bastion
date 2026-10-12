CREATE TABLE collaboration_folders (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
 scope ENUM('company','person') NOT NULL,
 person_id BIGINT UNSIGNED NULL,
 team_id BIGINT UNSIGNED NULL,
 parent_id BIGINT UNSIGNED NULL,
 name VARCHAR(120) NOT NULL,
 created_by BIGINT UNSIGNED NOT NULL,
 version BIGINT UNSIGNED NOT NULL DEFAULT 1,
 created_at DATETIME(6) NOT NULL,
 updated_at DATETIME(6) NOT NULL,
 deleted_at DATETIME(6) NULL,
 scope_key VARCHAR(40) CHARACTER SET ascii GENERATED ALWAYS AS (CASE WHEN scope='company' THEN 'company' ELSE CONCAT('person:',person_id) END) STORED,
 parent_key BIGINT UNSIGNED GENERATED ALWAYS AS (IFNULL(parent_id,0)) STORED,
 UNIQUE KEY collaboration_folder_name(scope_key,parent_key,name),
 INDEX collaboration_folder_scope(scope,person_id,team_id,deleted_at,id),
 FOREIGN KEY(person_id) REFERENCES users(id),
 FOREIGN KEY(team_id) REFERENCES teams(id),
 FOREIGN KEY(parent_id) REFERENCES collaboration_folders(id),
 FOREIGN KEY(created_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE collaboration_entries (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
 document_id BIGINT UNSIGNED NOT NULL UNIQUE,
 folder_id BIGINT UNSIGNED NULL,
 scope ENUM('company','person') NOT NULL,
 person_id BIGINT UNSIGNED NULL,
 team_id BIGINT UNSIGNED NULL,
 title VARCHAR(180) NOT NULL,
 notes TEXT NULL,
 uploaded_by BIGINT UNSIGNED NOT NULL,
 version BIGINT UNSIGNED NOT NULL DEFAULT 1,
 created_at DATETIME(6) NOT NULL,
 updated_at DATETIME(6) NOT NULL,
 INDEX collaboration_entry_scope(scope,person_id,team_id,updated_at,id),
 INDEX collaboration_entry_folder(folder_id,updated_at,id),
 FOREIGN KEY(document_id) REFERENCES documents(id),
 FOREIGN KEY(folder_id) REFERENCES collaboration_folders(id),
 FOREIGN KEY(person_id) REFERENCES users(id),
 FOREIGN KEY(team_id) REFERENCES teams(id),
 FOREIGN KEY(uploaded_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT IGNORE INTO permissions(name) VALUES
 ('colaboracion.view'),
 ('colaboracion.upload'),
 ('colaboracion.organize'),
 ('colaboracion.delete');
