-- Bind a pending administrative decision to the exact client document that
-- was requested. Existing rows without a snapshot fail closed on review.
ALTER TABLE oauth_client_requests
  ADD COLUMN metadata_hash BINARY(32) NULL;
