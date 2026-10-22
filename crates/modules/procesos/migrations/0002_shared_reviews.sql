CREATE TABLE procesos_marks (
  proceso_id VARCHAR(67) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  marked TINYINT(1) NOT NULL DEFAULT 0,
  note TEXT NOT NULL,
  version BIGINT UNSIGNED NOT NULL,
  updated_by BIGINT UNSIGNED NOT NULL,
  updated_at DATETIME(6) NOT NULL,
  PRIMARY KEY (proceso_id),
  KEY ix_procesos_marks_marked (marked, proceso_id),
  CONSTRAINT fk_procesos_marks_row FOREIGN KEY (proceso_id) REFERENCES procesos_rows(proceso_id) ON DELETE CASCADE,
  CONSTRAINT fk_procesos_marks_user FOREIGN KEY (updated_by) REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
