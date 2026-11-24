CREATE TABLE unplaced_scene_order (
    scene_id TEXT PRIMARY KEY REFERENCES scene(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK(position >= 0) UNIQUE
);

INSERT INTO unplaced_scene_order(scene_id, position)
SELECT scene.id,
       ROW_NUMBER() OVER (ORDER BY scene.created_at, scene.id) - 1
FROM scene
LEFT JOIN scene_placement ON scene_placement.scene_id = scene.id
WHERE scene_placement.scene_id IS NULL;
