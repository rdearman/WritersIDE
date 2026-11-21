CREATE TABLE scene_character_absence(
  scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
  character_id TEXT NOT NULL REFERENCES character(id) ON DELETE RESTRICT,
  notes TEXT NOT NULL DEFAULT '',
  PRIMARY KEY(scene_id,character_id)
);
CREATE INDEX scene_character_absence_character ON scene_character_absence(character_id,scene_id);
CREATE TRIGGER character_absence_conflicts_with_presence BEFORE INSERT ON scene_character_absence
WHEN EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=NEW.scene_id AND sc.character_id=NEW.character_id AND sc.participation_role IN('pov','major','minor','off_camera'))
BEGIN SELECT RAISE(ABORT,'character cannot be explicitly absent and present in the same scene'); END;
CREATE TRIGGER character_presence_conflicts_with_absence BEFORE INSERT ON scene_character
WHEN NEW.participation_role IN('pov','major','minor','off_camera') AND EXISTS(SELECT 1 FROM scene_character_absence a WHERE a.scene_id=NEW.scene_id AND a.character_id=NEW.character_id)
BEGIN SELECT RAISE(ABORT,'character cannot be explicitly absent and present in the same scene'); END;
CREATE TRIGGER character_presence_update_conflicts_with_absence BEFORE UPDATE OF scene_id,character_id,participation_role ON scene_character
WHEN NEW.participation_role IN('pov','major','minor','off_camera') AND EXISTS(SELECT 1 FROM scene_character_absence a WHERE a.scene_id=NEW.scene_id AND a.character_id=NEW.character_id)
BEGIN SELECT RAISE(ABORT,'character cannot be explicitly absent and present in the same scene'); END;
