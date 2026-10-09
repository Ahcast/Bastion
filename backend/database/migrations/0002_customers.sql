CREATE TABLE domain_events (id CHAR(32) CHARACTER SET ascii PRIMARY KEY, topic VARCHAR(80) CHARACTER SET ascii NOT NULL, schema_version INT NOT NULL, entity_type VARCHAR(60) NOT NULL, entity_id BIGINT UNSIGNED NOT NULL, entity_version BIGINT UNSIGNED NOT NULL, user_id BIGINT UNSIGNED NOT NULL, correlation_id VARCHAR(64) NOT NULL, created_at DATETIME(6) NOT NULL, FOREIGN KEY(user_id) REFERENCES users(id), INDEX domain_entity(entity_type,entity_id,created_at)) ENGINE=InnoDB;
CREATE TABLE customers (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
 name VARCHAR(180) NOT NULL,
 canonical_name VARCHAR(180) COLLATE utf8mb4_bin NOT NULL UNIQUE,
 rtn VARCHAR(30) NOT NULL DEFAULT '', canonical_rtn CHAR(14) CHARACTER SET ascii NULL UNIQUE,
 contact_name VARCHAR(120) NOT NULL DEFAULT '', phone VARCHAR(50) NOT NULL DEFAULT '', email VARCHAR(254) NOT NULL DEFAULT '', address VARCHAR(1000) NOT NULL DEFAULT '',
 institution_type VARCHAR(12) NOT NULL DEFAULT 'other', is_active BOOLEAN NOT NULL DEFAULT TRUE,
 created_by BIGINT UNSIGNED NOT NULL, updated_by BIGINT UNSIGNED NOT NULL, team_id BIGINT UNSIGNED NULL,
 version BIGINT UNSIGNED NOT NULL DEFAULT 1, created_at DATETIME(6) NOT NULL, updated_at DATETIME(6) NOT NULL,
 FOREIGN KEY(created_by) REFERENCES users(id), FOREIGN KEY(updated_by) REFERENCES users(id), FOREIGN KEY(team_id) REFERENCES teams(id),
 INDEX customer_owner(created_by,id), INDEX customer_team(team_id,id), INDEX customer_active(is_active,id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE customer_contacts (id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY, customer_id BIGINT UNSIGNED NOT NULL, name VARCHAR(120) NOT NULL, role VARCHAR(100) NOT NULL, phone VARCHAR(50) NOT NULL, email VARCHAR(254) NOT NULL, position INT NOT NULL, FOREIGN KEY(customer_id) REFERENCES customers(id), UNIQUE(customer_id,position)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE customer_addresses (id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY, customer_id BIGINT UNSIGNED NOT NULL, label VARCHAR(80) NOT NULL, address VARCHAR(1000) NOT NULL, position INT NOT NULL, FOREIGN KEY(customer_id) REFERENCES customers(id), UNIQUE(customer_id,position)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE customer_notes (id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY, customer_id BIGINT UNSIGNED NOT NULL, user_id BIGINT UNSIGNED NOT NULL, body TEXT NOT NULL, created_at DATETIME(6) NOT NULL, FOREIGN KEY(customer_id) REFERENCES customers(id), FOREIGN KEY(user_id) REFERENCES users(id), INDEX customer_note(customer_id,id)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT INTO permissions(name) VALUES('clientes.view'),('clientes.create'),('clientes.update');
