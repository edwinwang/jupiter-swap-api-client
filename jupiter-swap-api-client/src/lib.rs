use std::collections::HashMap;

use anyhow::anyhow;
use quote::{InternalQuoteRequest, QuoteRequest, QuoteResponse};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use swap::{SwapInstructionsResponse, SwapInstructionsResponseInternal, SwapRequest, SwapResponse};
use thiserror::Error;

pub mod quote;
pub mod route_plan_with_metadata;
pub mod serde_helpers;
pub mod swap;
pub mod transaction_config;

#[derive(Clone)]
pub struct JupiterSwapApiClient {
    pub base_path: String,
    pub client: Client,
}

#[derive(Debug, Error)]
pub enum JupiterError {
    #[error("Request failed with status code {status_code}: {msg}")]
    RequestFailed {
        status_code: reqwest::StatusCode,
        msg: String,
    },
    #[error("API error: {code} - {msg}")]
    ApiError { code: String, msg: String },
}

async fn check_status_code_and_deserialize<T: DeserializeOwned>(
    response: Response,
) -> Result<T, JupiterError> {
    if !response.status().is_success() {
        return Err(JupiterError::RequestFailed {
            status_code: response.status(),
            msg: response.text().await.unwrap_or_default(),
        });
    }

    let json_value =
        response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| JupiterError::RequestFailed {
                status_code: response.status(),
                msg: e.to_string(),
            })?;

    if let Some(error_msg) = json_value["error"].as_str() {
        let error_code = json_value["errorCode"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        return Err(JupiterError::ApiError {
            code: error_code,
            msg: error_msg.to_string(),
        });
    }

    serde_json::from_value(json_value).map_err(|e| JupiterError::RequestFailed {
        status_code: response.status(),
        msg: e.to_string(),
    })
}

impl JupiterSwapApiClient {
    pub fn new(base_path: String) -> Self {
        Self {
            base_path,
            client: Client::new(),
        }
    }

    pub async fn quote(&self, quote_request: &QuoteRequest) -> Result<QuoteResponse, JupiterError> {
        let url = format!("{}/quote", self.base_path);
        let extra_args = quote_request.quote_args.clone();
        let internal_quote_request = InternalQuoteRequest::from(quote_request.clone());
        let response = self
            .client
            .get(url)
            .query(&internal_quote_request)
            .query(&extra_args)
            .send()
            .await
            .map_err(|e| JupiterError::RequestFailed {
                status_code: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                msg: e.to_string(),
            })?;
        check_status_code_and_deserialize(response).await
    }

    pub async fn swap(
        &self,
        swap_request: &SwapRequest,
        extra_args: Option<HashMap<String, String>>,
    ) -> Result<SwapResponse, JupiterError> {
        let response = self
            .client
            .post(format!("{}/swap", self.base_path))
            .query(&extra_args)
            .json(swap_request)
            .send()
            .await
            .map_err(|e| JupiterError::RequestFailed {
                status_code: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                msg: e.to_string(),
            })?;
        check_status_code_and_deserialize(response).await
    }

    pub async fn swap_instructions(
        &self,
        swap_request: &SwapRequest,
    ) -> Result<SwapInstructionsResponse, JupiterError> {
        let response = self
            .client
            .post(format!("{}/swap-instructions", self.base_path))
            .json(swap_request)
            .send()
            .await
            .map_err(|e| JupiterError::RequestFailed {
                status_code: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                msg: e.to_string(),
            })?;
        check_status_code_and_deserialize::<SwapInstructionsResponseInternal>(response)
            .await
            .map(Into::into)
    }
}
