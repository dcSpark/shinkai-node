use uuid::Uuid;

use crate::models::Entity;

pub fn get_entity_by_key(entities: &Vec<Entity>, key: &str, value: &str) -> Option<Entity> {
    for entity in entities {
        match key {
            "id" => {
                if entity.id == value
                    || is_valid_uuid(value) && entity.id == Uuid::parse_str(value).unwrap().to_string().replace("-", "")
                {
                    return Some(entity.clone());
                }
            }
            "short_id" => {
                if entity.short_id.as_ref().unwrap_or(&"".to_string()) == value
                    || is_valid_uuid(value)
                        && entity.short_id.as_ref().unwrap_or(&"".to_string())
                            == Uuid::parse_str(value).unwrap().to_string().replace("-", "").as_str()
                {
                    return Some(entity.clone());
                }
            }
            "title" => {
                if entity.title == value {
                    return Some(entity.clone());
                }
            }
            "entity_type" => {
                if entity.entity_type.as_ref().unwrap_or(&"".to_string()) == value {
                    return Some(entity.clone());
                }
            }
            "description" => {
                if entity.description.as_ref().unwrap_or(&"".to_string()) == value {
                    return Some(entity.clone());
                }
            }
            _ => {}
        }
    }
    None
}

pub fn get_entity_by_name(entities: &Vec<Entity>, entity_name: &str) -> Vec<Entity> {
    entities
        .iter()
        .filter(|entity| entity.title == entity_name)
        .cloned()
        .collect()
}

pub fn is_valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}
