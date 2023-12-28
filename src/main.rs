use dotenv::dotenv;
use rocket::fs::FileServer;

#[macro_use]
extern crate rocket;

mod guards;
mod helpers;
mod utils;

mod db;
mod macros;
mod routes;
mod structs;

use routes::*;

#[launch]
fn rocket() -> _ {
    dotenv().ok();

    let rocket = mount_routes!(
        rocket::build(),
        index::index,
        user::new_user,
        test::test,
        variables::new_variable,
        projects::new_project,
        projects::get_variables,
        projects::add_user_to_project
    )
    .mount("/", FileServer::from("static/"));

    rocket
}
