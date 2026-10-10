CREATE TABLE suppliers (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
 name VARCHAR(180) NOT NULL, canonical_name VARCHAR(180) COLLATE utf8mb4_bin NOT NULL UNIQUE,
 rtn VARCHAR(30) NOT NULL DEFAULT '', canonical_rtn CHAR(14) CHARACTER SET ascii NULL UNIQUE,
 country VARCHAR(80) NOT NULL DEFAULT '', canonical_country VARCHAR(80) COLLATE utf8mb4_bin NOT NULL DEFAULT '', website VARCHAR(2048) NOT NULL DEFAULT '',
 contact_name VARCHAR(120) NOT NULL DEFAULT '', phone VARCHAR(50) NOT NULL DEFAULT '', email VARCHAR(254) NOT NULL DEFAULT '', address VARCHAR(1000) NOT NULL DEFAULT '',
 notes TEXT NOT NULL, payment_terms VARCHAR(1000) NOT NULL DEFAULT '', lead_time VARCHAR(120) NOT NULL DEFAULT '',
 evaluation TINYINT UNSIGNED NULL, reliability VARCHAR(12) NOT NULL DEFAULT 'unknown', response_hours INT UNSIGNED NULL,
 is_active BOOLEAN NOT NULL DEFAULT TRUE, created_by BIGINT UNSIGNED NOT NULL, updated_by BIGINT UNSIGNED NOT NULL, team_id BIGINT UNSIGNED NULL,
 version BIGINT UNSIGNED NOT NULL DEFAULT 1, created_at DATETIME(6) NOT NULL, updated_at DATETIME(6) NOT NULL,
 FOREIGN KEY(created_by) REFERENCES users(id), FOREIGN KEY(updated_by) REFERENCES users(id), FOREIGN KEY(team_id) REFERENCES teams(id),
 INDEX supplier_owner(created_by,id), INDEX supplier_team(team_id,id), INDEX supplier_active(is_active,id), INDEX supplier_country(canonical_country,is_active,id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE supplier_contacts (id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY, supplier_id BIGINT UNSIGNED NOT NULL, name VARCHAR(120) NOT NULL, role VARCHAR(100) NOT NULL, phone VARCHAR(50) NOT NULL, email VARCHAR(254) NOT NULL, position INT NOT NULL, FOREIGN KEY(supplier_id) REFERENCES suppliers(id), UNIQUE(supplier_id,position)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE supplier_tags (supplier_id BIGINT UNSIGNED NOT NULL, kind VARCHAR(12) CHARACTER SET ascii NOT NULL, name VARCHAR(80) NOT NULL, canonical_name VARCHAR(80) COLLATE utf8mb4_bin NOT NULL, position INT NOT NULL, FOREIGN KEY(supplier_id) REFERENCES suppliers(id), PRIMARY KEY(supplier_id,kind,canonical_name), UNIQUE(supplier_id,kind,position)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE supplier_products (id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY, supplier_id BIGINT UNSIGNED NOT NULL, name VARCHAR(120) NOT NULL, brand VARCHAR(80) NOT NULL, part_number VARCHAR(80) NOT NULL, description VARCHAR(1000) NOT NULL, lead_time VARCHAR(120) NOT NULL, moq DECIMAL(18,4) UNSIGNED NULL, position INT NOT NULL, FOREIGN KEY(supplier_id) REFERENCES suppliers(id), UNIQUE(supplier_id,position)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE supplier_notes (id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY, supplier_id BIGINT UNSIGNED NOT NULL, user_id BIGINT UNSIGNED NOT NULL, body TEXT NOT NULL, created_at DATETIME(6) NOT NULL, FOREIGN KEY(supplier_id) REFERENCES suppliers(id), FOREIGN KEY(user_id) REFERENCES users(id), INDEX supplier_note(supplier_id,id)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE supplier_price_observations (
 id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY, supplier_id BIGINT UNSIGNED NOT NULL,
 source_url VARCHAR(2048) NOT NULL, source_type VARCHAR(12) NOT NULL, source_observed_at DATETIME(6) NOT NULL,
 part_number VARCHAR(80) NOT NULL, observed_price DECIMAL(18,6) UNSIGNED NULL, currency CHAR(3) CHARACTER SET ascii NOT NULL DEFAULT '',
 availability VARCHAR(500) NOT NULL, lead_time VARCHAR(120) NOT NULL, moq DECIMAL(18,4) UNSIGNED NULL,
 taxes VARCHAR(12) NOT NULL, shipping VARCHAR(12) NOT NULL, user_id BIGINT UNSIGNED NOT NULL, captured_at DATETIME(6) NOT NULL,
 FOREIGN KEY(supplier_id) REFERENCES suppliers(id), FOREIGN KEY(user_id) REFERENCES users(id), INDEX supplier_price(supplier_id,id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT INTO permissions(name) VALUES('proveedores.view'),('proveedores.create'),('proveedores.update');
