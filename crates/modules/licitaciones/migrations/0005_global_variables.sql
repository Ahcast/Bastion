-- Existing form values remain intact for explicit review and migration.
ALTER TABLE licitaciones_expedientes ADD COLUMN variables_json JSON NULL;
