UPDATE project_search_content SET content=(
  SELECT s.synopsis||' '||s.purpose||' '||s.goal||' '||s.outcome||' '||s.status||' '||COALESCE(sc.content,'')||' '||COALESCE((
    SELECT group_concat(COALESCE(v.text_value,CAST(v.integer_value AS TEXT),v.decimal_value,CAST(v.boolean_value AS TEXT),v.entity_ref),' ')
    FROM custom_field_value v WHERE v.scene_id=s.id
  ),'')
  FROM scene s LEFT JOIN scene_search_content sc ON sc.scene_id=s.id WHERE s.id=project_search_content.entity_id
) WHERE entity_kind='scene';
DROP TRIGGER project_search_scene_update;
CREATE TRIGGER project_search_scene_update AFTER UPDATE OF title,synopsis,purpose,goal,outcome,status ON scene BEGIN
  UPDATE project_search_content SET title=NEW.title,content=NEW.synopsis||' '||NEW.purpose||' '||NEW.goal||' '||NEW.outcome||' '||NEW.status||' '||COALESCE((SELECT content FROM scene_search_content WHERE scene_id=NEW.id),'')||' '||COALESCE((SELECT group_concat(COALESCE(v.text_value,CAST(v.integer_value AS TEXT),v.decimal_value,CAST(v.boolean_value AS TEXT),v.entity_ref),' ') FROM custom_field_value v WHERE v.scene_id=NEW.id),'') WHERE entity_kind='scene' AND entity_id=NEW.id;
END;
DROP TRIGGER project_search_scene_text_update;
CREATE TRIGGER project_search_scene_text_update AFTER UPDATE OF content ON scene_search_content BEGIN
  UPDATE project_search_content SET content=(SELECT synopsis||' '||purpose||' '||goal||' '||outcome||' '||status||' '||NEW.content||' '||COALESCE((SELECT group_concat(COALESCE(v.text_value,CAST(v.integer_value AS TEXT),v.decimal_value,CAST(v.boolean_value AS TEXT),v.entity_ref),' ') FROM custom_field_value v WHERE v.scene_id=NEW.scene_id),'') FROM scene WHERE id=NEW.scene_id) WHERE entity_kind='scene' AND entity_id=NEW.scene_id;
END;
CREATE TRIGGER project_search_custom_field_insert AFTER INSERT ON custom_field_value BEGIN
  UPDATE project_search_content SET content=(
    SELECT s.synopsis||' '||s.purpose||' '||s.goal||' '||s.outcome||' '||s.status||' '||COALESCE(sc.content,'')||' '||COALESCE((
      SELECT group_concat(COALESCE(v.text_value,CAST(v.integer_value AS TEXT),v.decimal_value,CAST(v.boolean_value AS TEXT),v.entity_ref),' ')
      FROM custom_field_value v WHERE v.scene_id=s.id
    ),'')
    FROM scene s LEFT JOIN scene_search_content sc ON sc.scene_id=s.id WHERE s.id=NEW.scene_id
  ) WHERE entity_kind='scene' AND entity_id=NEW.scene_id;
END;
CREATE TRIGGER project_search_custom_field_update AFTER UPDATE ON custom_field_value BEGIN
  UPDATE project_search_content SET content=(
    SELECT s.synopsis||' '||s.purpose||' '||s.goal||' '||s.outcome||' '||s.status||' '||COALESCE(sc.content,'')||' '||COALESCE((
      SELECT group_concat(COALESCE(v.text_value,CAST(v.integer_value AS TEXT),v.decimal_value,CAST(v.boolean_value AS TEXT),v.entity_ref),' ')
      FROM custom_field_value v WHERE v.scene_id=s.id
    ),'')
    FROM scene s LEFT JOIN scene_search_content sc ON sc.scene_id=s.id WHERE s.id=NEW.scene_id
  ) WHERE entity_kind='scene' AND entity_id=NEW.scene_id;
  UPDATE project_search_content SET content=(
    SELECT s.synopsis||' '||s.purpose||' '||s.goal||' '||s.outcome||' '||s.status||' '||COALESCE(sc.content,'')||' '||COALESCE((
      SELECT group_concat(COALESCE(v.text_value,CAST(v.integer_value AS TEXT),v.decimal_value,CAST(v.boolean_value AS TEXT),v.entity_ref),' ')
      FROM custom_field_value v WHERE v.scene_id=s.id
    ),'')
    FROM scene s LEFT JOIN scene_search_content sc ON sc.scene_id=s.id WHERE s.id=OLD.scene_id
  ) WHERE OLD.scene_id<>NEW.scene_id AND entity_kind='scene' AND entity_id=OLD.scene_id;
END;
CREATE TRIGGER project_search_custom_field_delete AFTER DELETE ON custom_field_value BEGIN
  UPDATE project_search_content SET content=(
    SELECT s.synopsis||' '||s.purpose||' '||s.goal||' '||s.outcome||' '||s.status||' '||COALESCE(sc.content,'')||' '||COALESCE((
      SELECT group_concat(COALESCE(v.text_value,CAST(v.integer_value AS TEXT),v.decimal_value,CAST(v.boolean_value AS TEXT),v.entity_ref),' ')
      FROM custom_field_value v WHERE v.scene_id=s.id
    ),'')
    FROM scene s LEFT JOIN scene_search_content sc ON sc.scene_id=s.id WHERE s.id=OLD.scene_id
  ) WHERE entity_kind='scene' AND entity_id=OLD.scene_id;
END;
