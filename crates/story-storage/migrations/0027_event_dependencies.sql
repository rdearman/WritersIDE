CREATE TABLE story_event_dependency(
  predecessor_event_id TEXT NOT NULL REFERENCES story_event(id) ON DELETE CASCADE,
  successor_event_id TEXT NOT NULL REFERENCES story_event(id) ON DELETE CASCADE,
  dependency_kind TEXT NOT NULL CHECK(dependency_kind IN('precedes','requires')),
  notes TEXT NOT NULL DEFAULT '',
  PRIMARY KEY(predecessor_event_id,successor_event_id,dependency_kind),
  CHECK(predecessor_event_id<>successor_event_id)
);
CREATE INDEX story_event_dependency_successor ON story_event_dependency(successor_event_id,predecessor_event_id);
CREATE TRIGGER story_event_dependency_no_cycle BEFORE INSERT ON story_event_dependency
WHEN EXISTS(
  WITH RECURSIVE reachable(id) AS (
    SELECT successor_event_id FROM story_event_dependency WHERE predecessor_event_id=NEW.successor_event_id
    UNION
    SELECT d.successor_event_id FROM story_event_dependency d JOIN reachable r ON d.predecessor_event_id=r.id
  ) SELECT 1 FROM reachable WHERE id=NEW.predecessor_event_id
)
BEGIN SELECT RAISE(ABORT,'event dependency would create a cycle'); END;
