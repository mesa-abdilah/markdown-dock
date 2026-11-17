CREATE TABLE sync_state (
  file_id            TEXT PRIMARY KEY,
  root_uuid          TEXT NOT NULL,
  rel_path           TEXT NOT NULL,
  local_sha256       BLOB,
  last_synced_sha256 BLOB,
  blob_name          TEXT,
  blob_drive_id      TEXT,
  remote_md5         TEXT,
  status             TEXT NOT NULL CHECK (status IN
                     ('pending','uploading','synced','conflict','error','tombstoned')),
  attempts           INTEGER NOT NULL DEFAULT 0,
  next_attempt_at    INTEGER,
  last_error         TEXT,
  updated_at         INTEGER NOT NULL
);

CREATE INDEX idx_sync_pending ON sync_state(status, next_attempt_at);
CREATE INDEX idx_sync_root_path ON sync_state(root_uuid, rel_path);

CREATE TABLE sync_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE conflicts (
  id               INTEGER PRIMARY KEY,
  file_id          TEXT NOT NULL,
  root_uuid        TEXT NOT NULL,
  rel_path         TEXT NOT NULL,
  local_sha256     BLOB,
  remote_sha256    BLOB NOT NULL,
  remote_device    TEXT NOT NULL,
  remote_blob_name TEXT NOT NULL,
  remote_blob_id   TEXT NOT NULL,
  detected_at      INTEGER NOT NULL,
  resolution       TEXT CHECK (resolution IN ('keep_both','keep_local','keep_remote'))
);
