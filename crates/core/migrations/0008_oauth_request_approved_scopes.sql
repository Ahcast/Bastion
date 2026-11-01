-- Preserve exactly what an administrator approved for each request. Older
-- decisions remain NULL rather than implying that every scope was approved.
ALTER TABLE oauth_client_requests
  ADD COLUMN approved_scopes JSON NULL;
