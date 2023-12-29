# Template for routes

```rs
use crate::*;

pub async fn hello_world(State(state): State<AppState>) -> Result<&'static str, AppError> {
    Ok("Hello, World!")
}
```
