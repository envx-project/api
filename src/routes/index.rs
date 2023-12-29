use crate::*;

pub async fn hello_world(State(state): State<AppState>) -> Result<&'static str, AnyhowError> {
    let uuid = sqlx::query!("SELECT gen_random_uuid()")
        .fetch_one(&*state.db)
        .await?;

    println!("uuid: {:?}", uuid);

    Ok("Hello, World!")
}
