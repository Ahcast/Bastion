-- Existing private copies retain their object keys; new references have no private copy.
ALTER TABLE licitaciones_archivos MODIFY object_key VARCHAR(250) CHARACTER SET ascii COLLATE ascii_bin NULL;
