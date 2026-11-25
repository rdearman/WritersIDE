DROP INDEX content_unit_one_body_per_node;
CREATE UNIQUE INDEX content_unit_one_section_body_per_node
  ON content_unit(parent_node_id,kind) WHERE kind='section_body';

ALTER TABLE structural_removal_content_unit
  ADD COLUMN original_parent_node_id TEXT REFERENCES structural_node(id) ON DELETE RESTRICT;
ALTER TABLE structural_removal_content_unit
  ADD COLUMN original_position INTEGER CHECK(original_position IS NULL OR original_position>=0);
ALTER TABLE structural_removal_content_unit
  ADD COLUMN applied_parent_node_id TEXT REFERENCES structural_node(id) ON DELETE RESTRICT;
ALTER TABLE structural_removal_content_unit
  ADD COLUMN applied_position INTEGER CHECK(applied_position IS NULL OR applied_position>=0);
