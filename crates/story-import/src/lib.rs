use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceType {
    Scrivener,
    Spreadsheet,
    Unknown(String),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryItem {
    pub source_document_id: String,
    pub item_type: String,
    pub parent_source_id: Option<String>,
    pub source_sheet: Option<String>,
    pub source_row: Option<u32>,
    pub fields: BTreeMap<String, String>,
    pub unsupported: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportInventory {
    pub source_type: SourceType,
    pub source_identifier: String,
    pub source_fingerprint: String,
    pub items: Vec<InventoryItem>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldMapping {
    pub source_field: String,
    pub destination_entity: String,
    pub destination_field: String,
    pub conversion: Option<String>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MappingPlan {
    pub fields: Vec<FieldMapping>,
    pub unresolved: Vec<String>,
    pub decisions: BTreeMap<String, String>,
}
#[derive(Debug, Error, Eq, PartialEq)]
pub enum ImportError {
    #[error("duplicate source document identifier: {0}")]
    DuplicateSourceId(String),
    #[error("missing parent source item: {0}")]
    MissingParent(String),
    #[error("mapping refers to absent source field: {0}")]
    MissingField(String),
}

impl ImportInventory {
    /// Validates identity and hierarchy before any destination project is mutated.
    pub fn validate(&self) -> Result<(), ImportError> {
        let mut ids = BTreeSet::new();
        for item in &self.items {
            if !ids.insert(&item.source_document_id) {
                return Err(ImportError::DuplicateSourceId(
                    item.source_document_id.clone(),
                ));
            }
        }
        for item in &self.items {
            if let Some(parent) = &item.parent_source_id
                && !ids.contains(parent)
            {
                return Err(ImportError::MissingParent(parent.clone()));
            }
        }
        Ok(())
    }
    pub fn apply_mapping_plan(&self, plan: &MappingPlan) -> Result<usize, ImportError> {
        self.validate()?;
        let known: BTreeSet<&str> = self
            .items
            .iter()
            .flat_map(|i| i.fields.keys().map(String::as_str))
            .collect();
        for m in &plan.fields {
            if !known.contains(m.source_field.as_str()) {
                return Err(ImportError::MissingField(m.source_field.clone()));
            }
        }
        Ok(plan.fields.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn synthetic_fixture() -> ImportInventory {
        let bytes = b"synthetic source inventory fixture";
        ImportInventory {
            source_type: SourceType::Spreadsheet,
            source_identifier: "fixture.ods".into(),
            source_fingerprint: format!("{:x}", Sha256::digest(bytes)),
            items: vec![
                InventoryItem {
                    source_document_id: "Arc:row:2".into(),
                    item_type: "scene".into(),
                    parent_source_id: None,
                    source_sheet: Some("Arc".into()),
                    source_row: Some(2),
                    fields: BTreeMap::from([
                        ("Title".into(), "Opening".into()),
                        ("POV".into(), "Mara".into()),
                    ]),
                    unsupported: vec![],
                },
                InventoryItem {
                    source_document_id: "Arc:row:3".into(),
                    item_type: "scene".into(),
                    parent_source_id: Some("Arc:row:2".into()),
                    source_sheet: Some("Arc".into()),
                    source_row: Some(3),
                    fields: BTreeMap::from([("Title".into(), "Consequence".into())]),
                    unsupported: vec!["embedded chart styling".into()],
                },
            ],
            warnings: vec![],
        }
    }
    #[test]
    fn synthetic_inventory_and_mapping_are_validated() {
        let inv = synthetic_fixture();
        inv.validate().unwrap();
        let plan = MappingPlan {
            fields: vec![FieldMapping {
                source_field: "Title".into(),
                destination_entity: "scene".into(),
                destination_field: "title".into(),
                conversion: None,
            }],
            unresolved: vec!["POV".into()],
            decisions: BTreeMap::new(),
        };
        assert_eq!(inv.apply_mapping_plan(&plan).unwrap(), 1);
    }
    #[test]
    fn duplicate_source_items_are_rejected() {
        let mut inv = synthetic_fixture();
        inv.items[1].source_document_id = inv.items[0].source_document_id.clone();
        assert!(matches!(
            inv.validate(),
            Err(ImportError::DuplicateSourceId(_))
        ));
    }
}
