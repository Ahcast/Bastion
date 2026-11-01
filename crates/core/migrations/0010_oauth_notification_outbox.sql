ALTER TABLE admin_notifications
  ADD COLUMN mail_outbox_id BIGINT UNSIGNED NULL,
  ADD UNIQUE KEY uq_admin_notification_mail (mail_outbox_id),
  ADD CONSTRAINT fk_admin_notification_mail FOREIGN KEY (mail_outbox_id) REFERENCES mail_outbox(id) ON DELETE SET NULL;
