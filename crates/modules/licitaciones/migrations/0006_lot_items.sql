CREATE TABLE licitaciones_lotes (
    expediente_id BIGINT UNSIGNED NOT NULL,
    numero INT UNSIGNED NOT NULL,
    items_json JSON NOT NULL,
    PRIMARY KEY (expediente_id, numero),
    FOREIGN KEY (expediente_id) REFERENCES licitaciones_expedientes(id)
) ENGINE=InnoDB;
ALTER TABLE licitaciones_expedientes ADD COLUMN pdf_order_json JSON NULL;
