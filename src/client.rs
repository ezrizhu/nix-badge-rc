use anyhow::{bail, Result};
use core::str;
use embedded_svc::{
    http::{client::Client, Method},
    io::Read,
};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection};

use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub struct PersonId {
    pub id: u32,
}

#[derive(Debug, Deserialize)]
pub struct CheckInRecord {
    pub person: PersonId,
}

pub type CheckInData = Vec<CheckInRecord>;

// compile time fetch rc key
static KEY: &'static str = env!("KEY");

pub fn init() -> Result<Client<EspHttpConnection>> {
    let connection = EspHttpConnection::new(&Configuration {
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    })?;
    Ok(Client::wrap(connection))
}

pub fn get(client: &mut Client<EspHttpConnection>) -> Result<HashSet<u32>> {
    let auth_header = format!("Bearer {}", KEY);
    let headers = [
        ("accept", "application/json"),
        ("content-type", "application/json"),
        ("authorization", auth_header.as_str()),
    ];

    let request = client.request(
        Method::Get,
        "https://www.recurse.com/api/v1/hub_visits".as_ref(),
        &headers,
    )?;

    let response = request.submit()?;
    let status = response.status();

    match status {
        200..=299 => {
            let mut buf = Vec::new();
            let mut reader = response;

            loop {
                let mut chunk = [0_u8; 256];
                match Read::read(&mut reader, &mut chunk) {
                    Ok(0) => break,
                    Ok(size) => buf.extend_from_slice(&chunk[..size]),
                    Err(e) => bail!("Failed to read response: {}", e),
                }
            }

            let json_str = str::from_utf8(&buf)
                .map_err(|e| anyhow::anyhow!("Invalid UTF-8 response: {}", e))?;

            let check_in_data: CheckInData = serde_json::from_str(json_str)
                .map_err(|e| anyhow::anyhow!("Failed to deserialize JSON: {}", e))?;

            let ids_vec: Vec<u32> = check_in_data
                .into_iter()
                .map(|record| record.person.id)
                .collect();
            let ids_hashset = ids_vec.into_iter().collect();
            Ok(ids_hashset)
        }
        _ => bail!("Unexpected response code: {}", status),
    }
}
