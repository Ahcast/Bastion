CREATE TABLE colaboracion_files (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    scope VARCHAR(16) NOT NULL,
    id_persona BIGINT UNSIGNED NULL,
    folder_key VARCHAR(120) NOT NULL,
    title VARCHAR(255) NOT NULL,
    notes TEXT NOT NULL,
    filename VARCHAR(255) NOT NULL,
    size_bytes BIGINT UNSIGNED NOT NULL,
    mime VARCHAR(150) NOT NULL,
    trashed_at DATETIME(6) NULL,
    purge_after DATETIME(6) NULL,
    object_key VARCHAR(250) NOT NULL,
    content_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    actor_id BIGINT UNSIGNED NOT NULL,
    idempotency_key VARCHAR(100) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    state VARCHAR(16) NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY colaboracion_receipt (actor_id,idempotency_key),
    UNIQUE KEY colaboracion_object (object_key),
    KEY colaboracion_folder (folder_key,state,id),
    KEY colaboracion_cleanup (state,updated_at),
    KEY colaboracion_retention (state,purge_after),
    CONSTRAINT colaboracion_person FOREIGN KEY (id_persona) REFERENCES persons(id) ON DELETE RESTRICT,
    CONSTRAINT colaboracion_actor FOREIGN KEY (actor_id) REFERENCES users(id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE colaboracion_actor_limits (
    actor_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    window_start DATETIME(6) NOT NULL,
    requests INT UNSIGNED NOT NULL
) ENGINE=InnoDB;
