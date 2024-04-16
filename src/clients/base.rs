use reqwest::Client;
use std::collections::HashMap;
use url::Url;

use crate::utils::convert_to_query_params;

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8645";

pub struct BaseClient {
    base_url: String,
    client: Client,
}

impl BaseClient {
    pub fn new(base_url: Option<&str>) -> Self {
        let url = base_url.unwrap_or(DEFAULT_BASE_URL).to_string();
        let client = Client::new();
        BaseClient {
            base_url: url,
            client,
        }
    }

    pub async fn get(
        &self,
        url: &str,
        query_params: Option<HashMap<String, String>>,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let mut full_url = format!("{}/{}", self.base_url, url);

        // add query parameters
        if let Some(params) = query_params {
            let query_string = convert_to_query_params(params);
            full_url.push_str(&format!("?{}", query_string));
        }

        let res = self
            .client
            .get(&full_url)
            .header("Accept", "application/json, text/plain")
            .send()
            .await?;

        res.error_for_status()
    }

    pub async fn post(
        &self,
        url: &str,
        body: HashMap<&str, serde_json::Value>,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let full_url = format!("{}/{}", self.base_url, url);

        let res = self
            .client
            .post(&full_url)
            .header("Accept", "application/json, text/plain")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        res.error_for_status()
    }

    pub async fn delete(
        &self,
        url: &str,
        body: HashMap<&str, serde_json::Value>,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let full_url = format!("{}/{}", self.base_url, url);

        let res = self
            .client
            .delete(&full_url)
            .header("Accept", "application/json, text/plain")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        res.error_for_status()
    }
}
