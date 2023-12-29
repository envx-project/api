use sqlx::types::uuid::Uuid as SqlxUuid;
use uuid::Uuid;

pub trait UuidHelpers {
    fn to_sqlx(&self) -> SqlxUuid;
    fn to_uuid(&self) -> Uuid;
}

impl UuidHelpers for Uuid {
    fn to_sqlx(&self) -> SqlxUuid {
        SqlxUuid::parse_str(&self.to_string()).unwrap()
    }

    fn to_uuid(&self) -> Uuid {
        Uuid::parse_str(&self.to_string()).unwrap()
    }
}
