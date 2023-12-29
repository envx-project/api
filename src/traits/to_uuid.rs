use sqlx::types::Uuid;
pub trait ToUuid {
    fn to_uuid(&self) -> anyhow::Result<Uuid>;
}

impl ToUuid for String {
    fn to_uuid(&self) -> anyhow::Result<Uuid> {
        Ok(Uuid::parse_str(&self)?)
    }
}
