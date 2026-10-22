CREATE TABLE procesos_rows (
  proceso_id VARCHAR(67) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  entidad VARCHAR(500) NOT NULL,
  expediente VARCHAR(255) NOT NULL,
  fecha_recepcion DATETIME(6) NOT NULL,
  ultima_deteccion DATETIME(6) NOT NULL,
  puntaje TINYINT UNSIGNED NOT NULL,
  rubro VARCHAR(500) NOT NULL,
  resumen_corto MEDIUMTEXT NOT NULL,
  contenido_hash CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  source_json JSON NOT NULL,
  PRIMARY KEY (proceso_id),
  KEY ix_procesos_fecha (fecha_recepcion, proceso_id),
  KEY ix_procesos_entidad (entidad(150))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE procesos_actor_limits (
  actor_id BIGINT UNSIGNED NOT NULL,
  window_start DATETIME(6) NOT NULL,
  requests INT UNSIGNED NOT NULL,
  PRIMARY KEY (actor_id),
  CONSTRAINT fk_procesos_limits_actor FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE procesos_imports (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  actor_id BIGINT UNSIGNED NOT NULL,
  file_hash BINARY(32) NOT NULL,
  result_json JSON NOT NULL,
  created_at DATETIME(6) NOT NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uq_procesos_import_actor_hash (actor_id, file_hash),
  CONSTRAINT fk_procesos_import_actor FOREIGN KEY (actor_id) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
