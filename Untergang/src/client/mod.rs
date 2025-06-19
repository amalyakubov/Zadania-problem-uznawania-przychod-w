use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct IndividualClient {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone_number: String,
    pub pesel: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CompanyClient {
    pub name: String,
    pub address: String,
    pub email: String,
    pub phone_number: String,
    pub krs: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Client {
    #[serde(rename = "individual")]
    Individual(IndividualClient),
    #[serde(rename = "company")]
    Company(CompanyClient),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ClientId {
    #[serde(rename = "individual")]
    Individual(String),
    #[serde(rename = "company")]
    Company(String),
}
