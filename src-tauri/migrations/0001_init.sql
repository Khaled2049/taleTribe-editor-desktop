-- Timestamps are Unix milliseconds. remote_id, remote_revision and dirty are
-- unused until publish exists; they are here so it needs no table rewrite.

CREATE TABLE stories (
  id              TEXT PRIMARY KEY,
  title           TEXT NOT NULL,
  description     TEXT NOT NULL DEFAULT '',
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  remote_id       TEXT,
  remote_revision INTEGER,
  dirty           INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE chapters (
  id              TEXT PRIMARY KEY,
  story_id        TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
  title           TEXT NOT NULL,
  content         TEXT NOT NULL DEFAULT '',
  word_count      INTEGER NOT NULL DEFAULT 0,
  position        INTEGER NOT NULL,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  remote_id       TEXT,
  remote_revision INTEGER,
  dirty           INTEGER NOT NULL DEFAULT 1
) STRICT;
CREATE INDEX chapters_story_position ON chapters(story_id, position);

CREATE TABLE chapter_snapshots (
  id         TEXT PRIMARY KEY,
  chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
  content    TEXT NOT NULL,
  word_count INTEGER NOT NULL,
  created_at INTEGER NOT NULL
) STRICT;
CREATE INDEX chapter_snapshots_chapter ON chapter_snapshots(chapter_id, created_at);

CREATE TABLE characters (
  id              TEXT PRIMARY KEY,
  story_id        TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  age             INTEGER,
  soul            TEXT NOT NULL DEFAULT '',
  personality     TEXT NOT NULL DEFAULT '',
  voice           TEXT NOT NULL DEFAULT '',
  backstory       TEXT NOT NULL DEFAULT '',
  affiliations    TEXT NOT NULL DEFAULT '',
  notes           TEXT NOT NULL DEFAULT '',
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  remote_id       TEXT,
  remote_revision INTEGER,
  dirty           INTEGER NOT NULL DEFAULT 1
) STRICT;
CREATE INDEX characters_story ON characters(story_id);

CREATE TABLE places (
  id              TEXT PRIMARY KEY,
  story_id        TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  description     TEXT NOT NULL DEFAULT '',
  atmosphere      TEXT NOT NULL DEFAULT '',
  geography       TEXT NOT NULL DEFAULT '',
  history         TEXT NOT NULL DEFAULT '',
  significance    TEXT NOT NULL DEFAULT '',
  notes           TEXT NOT NULL DEFAULT '',
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  remote_id       TEXT,
  remote_revision INTEGER,
  dirty           INTEGER NOT NULL DEFAULT 1
) STRICT;
CREATE INDEX places_story ON places(story_id);
