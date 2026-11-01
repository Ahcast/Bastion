-- A pending consent must remain bound to the metadata reviewed when shown.
-- Existing pending requests without a snapshot are denied until recreated.
ALTER TABLE oauth_consent_requests ADD COLUMN approved_metadata_hash BINARY(32) NULL;
