-- Section-body prose is a stable content unit. Allow a section to receive
-- multiple prose documents when another section is removed and its content
-- is rehomed there; sibling positions still provide deterministic ordering.
DROP INDEX content_unit_one_section_body_per_node;
