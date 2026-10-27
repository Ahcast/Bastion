-- Empty, immutable schema library. No Typst/code or predefined business forms.
CREATE TABLE colaboracion_formularios (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    current_revision BIGINT UNSIGNED NOT NULL
) ENGINE=InnoDB;
CREATE TABLE colaboracion_formulario_versiones (
    form_id BIGINT UNSIGNED NOT NULL,
    revision BIGINT UNSIGNED NOT NULL,
    definition_json JSON NOT NULL,
    content_hash CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
    published_by BIGINT UNSIGNED NOT NULL,
    published_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (form_id,revision),
    FOREIGN KEY (form_id) REFERENCES colaboracion_formularios(id),
    FOREIGN KEY (published_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
CREATE TABLE colaboracion_formulario_recibos (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    response_json JSON NOT NULL,
    PRIMARY KEY (actor_id,key_hash),
    FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB;
