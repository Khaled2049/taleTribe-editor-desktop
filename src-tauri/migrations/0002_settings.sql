-- App preferences as JSON values by key. API keys are never stored here; they
-- live in the operating system's keychain.
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;
