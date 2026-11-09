CREATE TABLE procesos_tender_alerts (
  recipient_key CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  tender_id CHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  mail_outbox_id BIGINT UNSIGNED NOT NULL,
  queued_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (recipient_key, tender_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
