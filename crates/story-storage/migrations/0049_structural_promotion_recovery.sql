ALTER TABLE structural_removal_node ADD COLUMN original_parent_id TEXT REFERENCES structural_node(id) ON DELETE RESTRICT;
ALTER TABLE structural_removal_node ADD COLUMN original_position INTEGER CHECK(original_position IS NULL OR original_position>=0);
ALTER TABLE structural_removal_node ADD COLUMN applied_parent_id TEXT REFERENCES structural_node(id) ON DELETE RESTRICT;
ALTER TABLE structural_removal_node ADD COLUMN applied_position INTEGER CHECK(applied_position IS NULL OR applied_position>=0);
