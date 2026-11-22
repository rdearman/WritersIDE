CREATE TABLE diagnostic_suppression (
    id TEXT PRIMARY KEY,
    rule_id TEXT NOT NULL CHECK(length(trim(rule_id)) > 0),
    scope TEXT NOT NULL CHECK(scope IN ('rule', 'scene', 'finding')),
    scene_id TEXT,
    finding_fingerprint TEXT,
    reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
    created_unix_ms INTEGER NOT NULL CHECK(created_unix_ms >= 0),
    expires_unix_ms INTEGER CHECK(expires_unix_ms IS NULL OR expires_unix_ms > created_unix_ms),
    triggering_fingerprint TEXT,
    status TEXT NOT NULL CHECK(status IN ('active', 'stale', 'expired', 'revoked')) DEFAULT 'active',
    last_checked_unix_ms INTEGER NOT NULL CHECK(last_checked_unix_ms >= 0),
    CHECK (
        (scope = 'rule' AND scene_id IS NULL AND finding_fingerprint IS NULL) OR
        (scope = 'scene' AND scene_id IS NOT NULL AND finding_fingerprint IS NULL) OR
        (scope = 'finding' AND scene_id IS NOT NULL AND finding_fingerprint IS NOT NULL)
    )
);

CREATE INDEX diagnostic_suppression_lookup
    ON diagnostic_suppression(status, rule_id, scene_id, scope);
