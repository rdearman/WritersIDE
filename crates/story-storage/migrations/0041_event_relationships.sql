CREATE TABLE story_event_character(
    event_id TEXT NOT NULL REFERENCES story_event(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES character(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('participant','witness','cause','affected')),
    PRIMARY KEY(event_id,character_id,role)
);
CREATE INDEX story_event_character_character ON story_event_character(character_id,event_id);

CREATE TABLE story_event_location(
    event_id TEXT NOT NULL REFERENCES story_event(id) ON DELETE CASCADE,
    location_id TEXT NOT NULL REFERENCES location(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('occurred_at','starts_at','ends_at')),
    PRIMARY KEY(event_id,location_id,role)
);
CREATE INDEX story_event_location_location ON story_event_location(location_id,event_id);
