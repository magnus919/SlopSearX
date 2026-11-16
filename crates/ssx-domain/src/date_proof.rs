//! Closed publication-date evidence. This value is never dispatch authority.
use crate::DomainError;
use serde::{Deserialize, Serialize};
use ssx_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationDateProof {
    date_from: Option<String>,
    date_to: Option<String>,
}
impl PublicationDateProof {
    /// The application must first validate date bounds and audited OpenAlex scope.
    /// Private fields prevent independent edits to requested/applied/provenance.
    pub fn openalex(
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<Self, DomainError> {
        if date_from.is_none() && date_to.is_none() {
            return Err(DomainError("date proof requires an absolute bound"));
        }
        Ok(Self { date_from, date_to })
    }
    pub fn to_value(&self) -> Value {
        json!({
            "requested":{"date_from":self.date_from,"date_to":self.date_to,"time_range":null},
            "applied":{"date_from":self.date_from,"date_to":self.date_to},
            "status":"enforced",
            "reason":"Publication dates enforced by the audited OpenAlex adapter.",
            "enforced_by":["upstream:openalex"],
            "engines":["openalex"],
            "warnings":[]
        })
    }
    pub fn from_value(value: Value) -> Result<Self, DomainError> {
        let text = |name: &str| match value.get("requested").and_then(|v| v.get(name)) {
            Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            _ => Err(DomainError("invalid date proof bound")),
        };
        let proof = Self::openalex(text("date_from")?, text("date_to")?)?;
        if value != proof.to_value() {
            return Err(DomainError(
                "publication date proof shape or provenance mismatch",
            ));
        }
        Ok(proof)
    }
}
impl Serialize for PublicationDateProof {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for PublicationDateProof {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_value(Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
