CREATE TABLE manuscript_assembly(
 id TEXT PRIMARY KEY,
 title TEXT NOT NULL,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL
);
CREATE TABLE assembly_item(
 id TEXT PRIMARY KEY,
 assembly_id TEXT NOT NULL REFERENCES manuscript_assembly(id) ON DELETE CASCADE,
 position INTEGER NOT NULL CHECK(position>=0),
 scene_id TEXT REFERENCES scene(id) ON DELETE RESTRICT,
 content_unit_id TEXT REFERENCES content_unit(id) ON DELETE RESTRICT,
 included INTEGER NOT NULL DEFAULT 1 CHECK(included IN(0,1)),
 CHECK((scene_id IS NOT NULL)+(content_unit_id IS NOT NULL)=1),
 UNIQUE(assembly_id,position),
 UNIQUE(assembly_id,scene_id),
 UNIQUE(assembly_id,content_unit_id)
);
CREATE INDEX assembly_item_order ON assembly_item(assembly_id,position);
