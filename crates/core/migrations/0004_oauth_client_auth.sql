-- Fail closed for previously registered clients until their authentication
-- method is explicitly reviewed and configured.
ALTER TABLE oauth_clients
  ADD COLUMN token_auth_method VARCHAR(48) CHARACTER SET ascii COLLATE ascii_bin NOT NULL DEFAULT 'unconfigured';
