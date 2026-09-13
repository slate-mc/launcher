use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! domain_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            #[must_use]
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self::from_uuid(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.as_uuid()
            }
        }
    };
}

domain_id!(AccountId);
domain_id!(InstanceId);
domain_id!(JobId);
domain_id!(RequestId);
domain_id!(RevisionId);
domain_id!(SessionId);
domain_id!(ServerId);
domain_id!(StorageRootId);

#[cfg(test)]
mod tests {
    use super::InstanceId;

    #[test]
    fn ids_serialize_as_uuid_strings() -> Result<(), serde_json::Error> {
        let id = InstanceId::new();
        let encoded = serde_json::to_string(&id)?;

        assert_eq!(encoded, format!("\"{id}\""));
        Ok(())
    }
}
