use crate::*;

pub async fn hello_world() -> Result<&'static str, AnyhowError> {
    Ok("Hello, World!")
}
