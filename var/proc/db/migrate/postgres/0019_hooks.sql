CREATE TABLE hooks (
    id TEXT PRIMARY KEY NOT NULL,
    revision BIGINT NOT NULL,
    definition_json TEXT NOT NULL,
    enabled BIGINT NOT NULL CHECK (enabled IN (0, 1))
);
CREATE TABLE hook_revision (
    id BIGINT PRIMARY KEY CHECK (id = 1),
    revision BIGINT NOT NULL
);
INSERT INTO hook_revision VALUES (1, 0);
