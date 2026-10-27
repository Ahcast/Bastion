CREATE TABLE licitaciones_archivos (
    id CHAR(48) CHARACTER SET ascii COLLATE ascii_bin NOT NULL PRIMARY KEY,
    expediente_id BIGINT UNSIGNED NOT NULL,
    section VARCHAR(16) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    filename VARCHAR(255) NOT NULL,
    mime VARCHAR(100) NOT NULL,
    byte_size BIGINT UNSIGNED NOT NULL,
    sha256 BINARY(32) NOT NULL,
    source_id BIGINT UNSIGNED NULL,
    source_sha256 BINARY(32) NULL,
    object_key VARCHAR(250) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY licitaciones_archivo_object (object_key),
    KEY licitaciones_archivo_expediente (expediente_id,id),
    FOREIGN KEY (expediente_id) REFERENCES licitaciones_expedientes(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
CREATE TABLE licitaciones_archivo_recibos (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    expediente_id BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (actor_id,key_hash),
    FOREIGN KEY (actor_id) REFERENCES users(id),
    FOREIGN KEY (expediente_id) REFERENCES licitaciones_expedientes(id)
) ENGINE=InnoDB;
CREATE TABLE licitaciones_archivo_limpieza (
    object_key VARCHAR(250) CHARACTER SET ascii COLLATE ascii_bin NOT NULL PRIMARY KEY,
    expediente_id BIGINT UNSIGNED NOT NULL,
    byte_size BIGINT UNSIGNED NOT NULL,
    state VARCHAR(16) NOT NULL,
    reason VARCHAR(32) NOT NULL,
    available_at DATETIME(6) NOT NULL,
    KEY licitaciones_archivo_cleanup (available_at),
    KEY licitaciones_archivo_reservas (expediente_id),
    FOREIGN KEY (expediente_id) REFERENCES licitaciones_expedientes(id)
) ENGINE=InnoDB;
