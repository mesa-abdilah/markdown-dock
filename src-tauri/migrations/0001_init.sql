CREATE TABLE roots (
  id            INTEGER PRIMARY KEY,
  root_uuid     TEXT NOT NULL UNIQUE,
  path          TEXT NOT NULL UNIQUE,
  label         TEXT NOT NULL,
  enabled       INTEGER NOT NULL DEFAULT 1,
  backup        INTEGER NOT NULL DEFAULT 0,
  exclude_globs TEXT NOT NULL DEFAULT '[]',
  added_at      INTEGER NOT NULL
);

CREATE TABLE documents (
  id               INTEGER PRIMARY KEY,
  root_id          INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
  rel_path         TEXT NOT NULL,
  file_name        TEXT NOT NULL,
  title            TEXT NOT NULL,
  tags_text        TEXT NOT NULL DEFAULT '',
  frontmatter_json TEXT,
  body             TEXT NOT NULL,
  size             INTEGER NOT NULL,
  mtime_ns         INTEGER NOT NULL,
  sha256           BLOB NOT NULL CHECK (length(sha256) = 32),
  word_count       INTEGER NOT NULL DEFAULT 0,
  indexed_at       INTEGER NOT NULL,
  UNIQUE (root_id, rel_path)
);

CREATE INDEX idx_documents_mtime ON documents(mtime_ns DESC);
CREATE INDEX idx_documents_root ON documents(root_id);

CREATE TABLE tags (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE document_tags (
  doc_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  tag_id INTEGER NOT NULL REFERENCES tags(id)      ON DELETE CASCADE,
  PRIMARY KEY (doc_id, tag_id)
) WITHOUT ROWID;

CREATE VIRTUAL TABLE documents_fts USING fts5(
  file_name,
  title,
  tags_text,
  body,
  content='documents',
  content_rowid='id',
  tokenize="unicode61 remove_diacritics 2",
  prefix='2 3 4'
);

CREATE VIRTUAL TABLE documents_trgm USING fts5(
  file_name,
  title,
  tags_text,
  content='documents',
  content_rowid='id',
  tokenize="trigram case_sensitive 0"
);

CREATE TRIGGER documents_ai AFTER INSERT ON documents BEGIN
  INSERT INTO documents_fts(rowid, file_name, title, tags_text, body)
  VALUES (new.id, new.file_name, new.title, new.tags_text, new.body);
  INSERT INTO documents_trgm(rowid, file_name, title, tags_text)
  VALUES (new.id, new.file_name, new.title, new.tags_text);
END;

CREATE TRIGGER documents_ad AFTER DELETE ON documents BEGIN
  INSERT INTO documents_fts(documents_fts, rowid, file_name, title, tags_text, body)
  VALUES ('delete', old.id, old.file_name, old.title, old.tags_text, old.body);
  INSERT INTO documents_trgm(documents_trgm, rowid, file_name, title, tags_text)
  VALUES ('delete', old.id, old.file_name, old.title, old.tags_text);
END;

CREATE TRIGGER documents_au AFTER UPDATE ON documents BEGIN
  INSERT INTO documents_fts(documents_fts, rowid, file_name, title, tags_text, body)
  VALUES ('delete', old.id, old.file_name, old.title, old.tags_text, old.body);
  INSERT INTO documents_trgm(documents_trgm, rowid, file_name, title, tags_text)
  VALUES ('delete', old.id, old.file_name, old.title, old.tags_text);
  INSERT INTO documents_fts(rowid, file_name, title, tags_text, body)
  VALUES (new.id, new.file_name, new.title, new.tags_text, new.body);
  INSERT INTO documents_trgm(rowid, file_name, title, tags_text)
  VALUES (new.id, new.file_name, new.title, new.tags_text);
END;

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
