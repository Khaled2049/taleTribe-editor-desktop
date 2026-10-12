-- A record deleted here after it was published. The next publish removes it
-- from the website and then drops the row.
CREATE TABLE tombstones (
  kind            TEXT NOT NULL CHECK (kind IN ('chapter', 'character', 'place')),
  remote_id       TEXT NOT NULL,
  story_id        TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
  remote_revision INTEGER NOT NULL,
  label           TEXT NOT NULL,
  PRIMARY KEY (kind, remote_id)
) STRICT;

-- Where a local image was uploaded for a given website story, so a repeat
-- publish reuses the upload.
CREATE TABLE asset_uploads (
  asset           TEXT NOT NULL,
  remote_story_id TEXT NOT NULL,
  url             TEXT NOT NULL,
  PRIMARY KEY (asset, remote_story_id)
) STRICT;
