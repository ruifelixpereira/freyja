// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use async_trait::async_trait;
use freyja_common::{config_utils, out_dir};
use reqwest::Client;

use crate::config::Config;
use freyja_contracts::digital_twin_adapter::{
    DigitalTwinAdapter, DigitalTwinAdapterError, GetDigitalTwinProviderRequest,
    GetDigitalTwinProviderResponse,
};
use mock_digital_twin::ENTITY_QUERY_PATH;

const CONFIG_FILE_STEM: &str = "mock_digital_twin_adapter_config";

/// Mocks a Digital Twin Adapter that calls the mocks/mock_digital_twin
/// to get entity access info.
pub struct MockDigitalTwinAdapter {
    /// The adapter config
    config: Config,

    /// Async Reqwest HTTP Client
    client: Client,
}

impl MockDigitalTwinAdapter {
    /// Creates a new MockDigitalTwinAdapter with the specified config
    ///
    /// # Arguments
    /// - `config`: the config to use
    pub fn from_config(config: Config) -> Result<Self, DigitalTwinAdapterError> {
        Ok(Self {
            config,
            client: Client::new(),
        })
    }

    /// Helper to map HTTP error codes to our own error type
    ///
    /// # Arguments
    /// - `error`: the HTTP error to translate
    fn map_status_err(error: reqwest::Error) -> DigitalTwinAdapterError {
        match error.status() {
            Some(reqwest::StatusCode::NOT_FOUND) => {
                DigitalTwinAdapterError::entity_not_found(error)
            }
            _ => DigitalTwinAdapterError::communication(error),
        }
    }
}

#[async_trait]
impl DigitalTwinAdapter for MockDigitalTwinAdapter {
    /// Creates a new instance of a MockDigitalTwinAdapter
    fn create_new() -> Result<Self, DigitalTwinAdapterError> {
        let config = config_utils::read_from_files(
            CONFIG_FILE_STEM,
            config_utils::JSON_EXT,
            out_dir!(),
            DigitalTwinAdapterError::io,
            DigitalTwinAdapterError::deserialize,
        )?;

        Self::from_config(config)
    }

    /// Gets the info of an entity via an HTTP request.
    ///
    /// # Arguments
    /// - `request`: the request to send to the mock digital twin server
    async fn find_by_id(
        &self,
        request: GetDigitalTwinProviderRequest,
    ) -> Result<GetDigitalTwinProviderResponse, DigitalTwinAdapterError> {
        let target = format!(
            "{}{ENTITY_QUERY_PATH}{}",
            self.config.digital_twin_service_uri, request.entity_id
        );

        self.client
            .get(&target)
            .send()
            .await
            .map_err(DigitalTwinAdapterError::communication)?
            .error_for_status()
            .map_err(Self::map_status_err)?
            .json::<GetDigitalTwinProviderResponse>()
            .await
            .map_err(DigitalTwinAdapterError::deserialize)
    }
}
