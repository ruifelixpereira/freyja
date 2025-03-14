// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use async_trait::async_trait;

use crate::entity::Entity;

/// Manages a collection of proxies and provides access to them.
/// Conceptually similar to a gateway for the proxies.
#[async_trait]
pub trait ProviderProxySelector {
    /// Updates an existing proxy for an entity if possible,
    /// otherwise creates a new proxy to handle that entity.
    ///
    /// # Arguments
    /// - `entity`: the entity that the proxy should handle
    async fn create_or_update_proxy(
        &mut self,
        entity: &Entity,
    ) -> Result<(), ProviderProxySelectorError>;

    /// Requests that the value of an entity be published as soon as possible
    ///
    /// # Arguments
    /// - `entity_id`: the entity to request
    async fn request_entity_value(
        &mut self,
        entity_id: &str,
    ) -> Result<(), ProviderProxySelectorError>;
}

proc_macros::error! {
    ProviderProxySelectorError {
        ProviderProxyError,
        EntityNotFound,
        ProtocolNotSupported,
        OperationNotSupported,
        Io,
        Serialize,
        Deserialize,
        Communication,
        Unknown
    }
}
