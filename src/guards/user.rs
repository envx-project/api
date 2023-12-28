use crate::structs::User;
use anyhow::{anyhow, Error};
use rocket::{
    http::Status,
    request::Outcome,
    request::{self, FromRequest, Request},
};

use pgp::{composed::message::Message, Deserializable};

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, ()> {
        let authorization = match req.headers().get_one("Authorization") {
            Some(authorization) => authorization,
            None => return Outcome::Error((Status::Unauthorized, ())),
        };

        let (id, signed_challenge) = match authorization.split_once(':') {
            Some((id, signed_challenge)) => (id, signed_challenge),
            None => return Outcome::Error((Status::Unauthorized, ())),
        };

        let signed_challenge = Message::from_string(&signed_challenge).unwrap().0;

        let challenge =
            std::str::from_utf8(&signed_challenge.get_content().unwrap().unwrap()).unwrap();

        let challenge_date = Date
    }
}
