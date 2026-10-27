-- Existing expedients keep their ID, state, version and contents unchanged.
ALTER TABLE licitaciones_expedientes
    ADD COLUMN name VARCHAR(255) NOT NULL DEFAULT '';
