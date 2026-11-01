CREATE TABLE licitaciones_formularios_instalados (
    form_key VARCHAR(32) CHARACTER SET ascii COLLATE ascii_bin NOT NULL PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT TRUE
) ENGINE=InnoDB;
INSERT INTO licitaciones_formularios_instalados (form_key, enabled) VALUES ('integridad', TRUE), ('oferente', TRUE);
