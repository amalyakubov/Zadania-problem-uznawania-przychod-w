use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct IndividualClient {
    first_name: String,
    last_name: String,
    email: String,
    phone_nummber: String,
    pesel: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CompanyClient {
    name: String,
    address: String,
    email: String,
    phone_number: String,
    krs: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Client {
    #[serde(rename = "individual")]
    Individual(IndividualClient),
    #[serde(rename = "company")]
    Company(CompanyClient),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientId(u64);
