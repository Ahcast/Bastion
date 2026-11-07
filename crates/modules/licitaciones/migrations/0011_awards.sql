CREATE TABLE licitaciones_adjudicaciones (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    expediente_id BIGINT UNSIGNED NOT NULL,
    snapshot_json JSON NOT NULL,
    created_by BIGINT UNSIGNED NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY adjudicaciones_expediente (expediente_id,id),
    FOREIGN KEY (expediente_id) REFERENCES licitaciones_expedientes(id),
    FOREIGN KEY (created_by) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE TABLE licitaciones_adjudicacion_items (
    adjudicacion_id BIGINT UNSIGNED NOT NULL,
    expediente_id BIGINT UNSIGNED NOT NULL,
    lote INT UNSIGNED NOT NULL,
    item_numero INT UNSIGNED NOT NULL,
    cantidad DECIMAL(24,12) NOT NULL,
    PRIMARY KEY (adjudicacion_id,lote,item_numero),
    KEY adjudicacion_cantidades (expediente_id,lote,item_numero),
    FOREIGN KEY (adjudicacion_id) REFERENCES licitaciones_adjudicaciones(id),
    FOREIGN KEY (expediente_id) REFERENCES licitaciones_expedientes(id)
) ENGINE=InnoDB;
CREATE TABLE licitaciones_adjudicacion_recibos (
    actor_id BIGINT UNSIGNED NOT NULL,
    key_hash BINARY(32) NOT NULL,
    request_hash BINARY(32) NOT NULL,
    adjudicacion_id BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (actor_id,key_hash),
    FOREIGN KEY (actor_id) REFERENCES users(id),
    FOREIGN KEY (adjudicacion_id) REFERENCES licitaciones_adjudicaciones(id)
) ENGINE=InnoDB;
