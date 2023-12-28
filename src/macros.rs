#[macro_export]
macro_rules! mount_routes {
    ($rocket:expr, $($route:path),+) => {
        $rocket.mount("/", routes![$($route),+])
    };
}
