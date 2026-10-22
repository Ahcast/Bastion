ALTER TABLE procesos_imports
  MODIFY actor_id BIGINT UNSIGNED NULL,
  ADD source_key VARCHAR(32) CHARACTER SET ascii COLLATE ascii_bin NULL AFTER actor_id,
  ADD UNIQUE KEY uq_procesos_import_source_hash (source_key, file_hash);
