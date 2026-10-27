CREATE TABLE licitaciones_expedientes (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    version BIGINT UNSIGNED NOT NULL DEFAULT 1,
    state VARCHAR(16) CHARACTER SET ascii COLLATE ascii_bin NOT NULL DEFAULT 'borrador',
    created_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_by BIGINT UNSIGNED NOT NULL,
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    deleted_at DATETIME(6) NULL,
    KEY licitaciones_listing (deleted_at,id),
    FOREIGN KEY (created_by) REFERENCES users(id),
    FOREIGN KEY (updated_by) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE TABLE licitaciones_formularios (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    expediente_id BIGINT UNSIGNED NOT NULL,
    source_json JSON NOT NULL,
    values_json JSON NOT NULL,
    KEY licitaciones_copias (expediente_id,id),
    FOREIGN KEY (expediente_id) REFERENCES licitaciones_expedientes(id)
) ENGINE=InnoDB;
CREATE TABLE licitaciones_recibos (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    expediente_id BIGINT UNSIGNED NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY (actor_id,key_hash),
    FOREIGN KEY (actor_id) REFERENCES users(id),
    FOREIGN KEY (expediente_id) REFERENCES licitaciones_expedientes(id)
) ENGINE=InnoDB;
CREATE TABLE licitaciones_actor_limits (
    actor_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    window_start DATETIME(6) NOT NULL,
    requests INT UNSIGNED NOT NULL,
    FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB;
