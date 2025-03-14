// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU8, Ordering},
    sync::Arc,
    sync::Mutex,
    time::Duration,
};

use async_trait::async_trait;
use crossbeam::queue::SegQueue;
use freyja_common::{config_utils, out_dir};
use log::info;

use crate::config::{Config, EntityConfig};
use freyja_contracts::provider_proxy::{ProviderProxy, ProviderProxyError, SignalValue};

const CONFIG_FILE_STEM: &str = "in_memory_mock_proxy_config";
const GET_OPERATION: &str = "Get";
const SUBSCRIBE_OPERATION: &str = "Subscribe";
const SUPPORTED_OPERATIONS: &[&str] = &[GET_OPERATION, SUBSCRIBE_OPERATION];

#[derive(Debug)]
pub struct InMemoryMockProviderProxy {
    /// Maps the number of calls to each provider so we can mock changing behavior
    data: HashMap<String, (EntityConfig, AtomicU8)>,

    /// Local cache for keeping track of which entities this provider proxy contains
    entity_operation_map: Mutex<HashMap<String, String>>,

    /// Shared queue for all proxies to push new signal values of entities
    signal_values_queue: Arc<SegQueue<SignalValue>>,

    /// The frequency between updates to signal values
    signal_update_frequency: Duration,
}

impl InMemoryMockProviderProxy {
    /// Creates a new InMemoryMockDigitalTwinAdapter with the specified config
    ///
    /// # Arguments
    /// - `config`: the config to use
    /// - `signal_values_queue`: shared queue for all proxies to push new signal values of entities
    /// - `interval_between_signal_generation_ms`: the interval in milliseconds between signal value generation
    pub fn from_config(
        config: Config,
        signal_values_queue: Arc<SegQueue<SignalValue>>,
    ) -> Result<Self, ProviderProxyError> {
        Ok(Self {
            entity_operation_map: Mutex::new(HashMap::new()),
            data: config
                .entities
                .into_iter()
                .map(|c| (c.entity_id.clone(), (c, AtomicU8::new(0))))
                .collect(),
            signal_values_queue,
            signal_update_frequency: Duration::from_millis(config.signal_update_frequency_ms),
        })
    }

    /// Generates signal value for an entity id
    ///
    /// # Arguments
    /// - `entity_id`: the entity id that needs a signal value
    /// - `signal_values_queue`: shared queue for all proxies to push new signal values of entities
    /// - `data`: the current data of a provider
    fn generate_signal_value(
        entity_id: &str,
        signal_values_queue: Arc<SegQueue<SignalValue>>,
        data: &HashMap<String, (EntityConfig, AtomicU8)>,
    ) -> Result<(), ProviderProxyError> {
        let (entity_config, counter) = data
            .get(entity_id)
            .ok_or_else(|| format!("Cannot find {entity_id}"))
            .map_err(ProviderProxyError::entity_not_found)?;
        let n = counter.fetch_add(1, Ordering::SeqCst);

        let value = entity_config.values.get_nth(n).to_string();
        let entity_id = String::from(entity_id);

        let new_signal_value = SignalValue { entity_id, value };
        signal_values_queue.push(new_signal_value);
        Ok(())
    }
}

#[async_trait]
impl ProviderProxy for InMemoryMockProviderProxy {
    /// Creates a provider proxy
    ///
    /// # Arguments
    /// - `provider_uri`: the provider uri for accessing an entity's information
    /// - `signal_values_queue`: shared queue for all proxies to push new signal values of entities
    fn create_new(
        _provider_uri: &str,
        signal_values_queue: Arc<SegQueue<SignalValue>>,
    ) -> Result<Box<dyn ProviderProxy + Send + Sync>, ProviderProxyError>
    where
        Self: Sized,
    {
        let config = config_utils::read_from_files(
            CONFIG_FILE_STEM,
            config_utils::JSON_EXT,
            out_dir!(),
            ProviderProxyError::io,
            ProviderProxyError::deserialize,
        )?;

        Self::from_config(config, signal_values_queue).map(|r| Box::new(r) as _)
    }

    /// Runs a provider proxy
    async fn run(&self) -> Result<(), ProviderProxyError> {
        info!("Started an InMemoryMockProviderProxy!");

        loop {
            let entities_with_subscribe: Vec<String>;

            {
                entities_with_subscribe = self
                    .entity_operation_map
                    .lock()
                    .unwrap()
                    .clone()
                    .into_iter()
                    .filter(|(_, operation)| *operation == SUBSCRIBE_OPERATION)
                    .map(|(entity_id, _)| entity_id)
                    .collect();
            }

            for entity_id in entities_with_subscribe {
                let _ = Self::generate_signal_value(
                    &entity_id,
                    self.signal_values_queue.clone(),
                    &self.data,
                );
            }

            tokio::time::sleep(self.signal_update_frequency).await;
        }
    }

    /// Sends a request to a provider for obtaining the value of an entity
    ///
    /// # Arguments
    /// - `entity_id`: the entity id that needs a value
    async fn send_request_to_provider(&self, entity_id: &str) -> Result<(), ProviderProxyError> {
        let operation_result;
        {
            let lock = self.entity_operation_map.lock().unwrap();
            operation_result = lock.get(entity_id).cloned();
        }

        if operation_result.is_none() {
            return Err(ProviderProxyError::unknown(format!(
                "Entity {entity_id} does not have an operation registered"
            )));
        }

        // Only need to handle Get operations since subscribe has already happened
        let operation = operation_result.unwrap();
        if operation == GET_OPERATION {
            let _ = Self::generate_signal_value(
                entity_id,
                self.signal_values_queue.clone(),
                &self.data,
            );
        }

        Ok(())
    }

    /// Registers an entity id to a local cache inside a provider proxy to keep track of which entities a provider proxy contains.
    /// If the operation is Subscribe for an entity, the expectation is subscribe will happen in this function after registering an entity.
    ///
    /// # Arguments
    /// - `entity_id`: the entity id to add
    /// - `operation`: the operation that this entity supports
    async fn register_entity(
        &self,
        entity_id: &str,
        operation: &str,
    ) -> Result<(), ProviderProxyError> {
        self.entity_operation_map
            .lock()
            .unwrap()
            .insert(String::from(entity_id), String::from(operation));
        Ok(())
    }

    /// Checks if the operation is supported
    ///
    /// # Arguments
    /// - `operation`: check to see if this operation is supported by this provider proxy
    fn is_operation_supported(operation: &str) -> bool {
        SUPPORTED_OPERATIONS.contains(&operation)
    }
}

#[cfg(test)]
mod in_memory_mock_digital_twin_adapter_tests {
    use super::*;

    use crate::config::SensorValueConfig;

    #[test]
    fn can_create_new() {
        let signal_values_queue = Arc::new(SegQueue::new());
        let result = InMemoryMockProviderProxy::create_new("FAKE_URI", signal_values_queue);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn get_signal_value_returns_correct_values() {
        const STATIC_ID: &str = "static";
        const INCREASING_ID: &str = "increasing";
        const DECREASING_ID: &str = "decreasing";

        let (start, end, delta) = (0.0, 5.0, 1.0);
        let config = Config {
            signal_update_frequency_ms: 1000,
            entities: vec![
                EntityConfig {
                    entity_id: String::from(STATIC_ID),
                    values: SensorValueConfig::Static(42.0),
                },
                EntityConfig {
                    entity_id: String::from(INCREASING_ID),
                    values: SensorValueConfig::Stepwise { start, end, delta },
                },
                EntityConfig {
                    entity_id: String::from(DECREASING_ID),
                    values: SensorValueConfig::Stepwise {
                        start,
                        end: -end,
                        delta: -delta,
                    },
                },
            ],
        };

        let signal_values_queue = Arc::new(SegQueue::new());
        let in_memory_mock_provider_proxy =
            InMemoryMockProviderProxy::from_config(config, signal_values_queue.clone()).unwrap();

        const END_OF_SENSOR_VALUE_CONFIG_ITERATION: i32 = 5;

        // First for loop, we generate signal values for each entity until we've reached the end value of each
        // entity that has the stepwise functionality configured.
        for i in 0..END_OF_SENSOR_VALUE_CONFIG_ITERATION {
            let result = InMemoryMockProviderProxy::generate_signal_value(
                STATIC_ID,
                signal_values_queue.clone(),
                &in_memory_mock_provider_proxy.data,
            );
            assert!(result.is_ok());

            let static_value = signal_values_queue.pop().unwrap();
            assert_eq!(static_value.entity_id, STATIC_ID);
            assert_eq!(static_value.value.parse::<f32>().unwrap(), 42.0);

            let result = InMemoryMockProviderProxy::generate_signal_value(
                INCREASING_ID,
                signal_values_queue.clone(),
                &in_memory_mock_provider_proxy.data,
            );
            assert!(result.is_ok());

            let increasing_value = signal_values_queue.pop().unwrap();
            assert_eq!(increasing_value.entity_id, INCREASING_ID);
            assert_eq!(
                increasing_value.value.parse::<f32>().unwrap(),
                start + delta * i as f32
            );

            let result = InMemoryMockProviderProxy::generate_signal_value(
                DECREASING_ID,
                signal_values_queue.clone(),
                &in_memory_mock_provider_proxy.data,
            );
            assert!(result.is_ok());

            let decreasing_value = signal_values_queue.pop().unwrap();
            assert_eq!(decreasing_value.entity_id, DECREASING_ID);
            assert_eq!(
                decreasing_value.value.parse::<f32>().unwrap(),
                start - delta * i as f32
            );
        }

        // Validating each entity that has the stepwise functionality configured is at its end value
        for _ in 0..END_OF_SENSOR_VALUE_CONFIG_ITERATION {
            let result = InMemoryMockProviderProxy::generate_signal_value(
                STATIC_ID,
                signal_values_queue.clone(),
                &in_memory_mock_provider_proxy.data,
            );
            assert!(result.is_ok());

            let static_value = signal_values_queue.pop().unwrap();
            assert_eq!(static_value.entity_id, STATIC_ID);
            assert_eq!(static_value.value.parse::<f32>().unwrap(), 42.0);

            let result = InMemoryMockProviderProxy::generate_signal_value(
                INCREASING_ID,
                signal_values_queue.clone(),
                &in_memory_mock_provider_proxy.data,
            );
            assert!(result.is_ok());

            let increasing_value = signal_values_queue.pop().unwrap();
            assert_eq!(increasing_value.entity_id, INCREASING_ID);
            assert_eq!(increasing_value.value.parse::<f32>().unwrap(), end);

            let result = InMemoryMockProviderProxy::generate_signal_value(
                DECREASING_ID,
                signal_values_queue.clone(),
                &in_memory_mock_provider_proxy.data,
            );
            assert!(result.is_ok());

            let decreasing_value = signal_values_queue.pop().unwrap();
            assert_eq!(decreasing_value.entity_id, DECREASING_ID);
            assert_eq!(decreasing_value.value.parse::<f32>().unwrap(), -end);
        }
    }
}
