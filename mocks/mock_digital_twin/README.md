# Mock Digital Twin

The Mock Digital Twin mocks the behavior of the in-vehicle digital twin services (e.g. Ibeji) as a separate application. This enables functionality similar to the in-memory mock, but with finer control over the behavior of the mocked data.

## Configuration

This mock supports the following configuration:

- `digital_twin_server_authority`: The authority that will be used for hosting the mock digital twin service
- `entities`: A list of entities with the following properties:
  - `begin`: An integer indicating when to enable this entity (refer to [Behavior](#behavior) for more information on how this value is used)
  - `end`: An optional integer indicating when to disable this entity. Set to `null` if you never want the entity to "turn off" (refer to [Behavior](#behavior) for more information on how this value is used)
  - `entity`: Describes an entity that should be provided via the `find_by_id` API at some point during the application's lifecycle. The properties of this object are identical to those of the [In-Memory Mock Digital Twin Adapter](../../digital_twin_adapters/in_memory_mock_digital_twin_adapter/README.md) with the following modifications:
    - `protocol`: When using this service, this value should always be `http`
  - `values`: Defines the values that the entity's signal should take. This can take one of two forms:
    - `Static`: The signal's value is a constant, configured as a string
    - `Stepwise`: The signal's value will change, increasing and decreasing cyclically by a set value between and upper and lower bound. When using this setting the following additional configuration is required:
      - `start`: The starting value of the signal. This can be either the upper or lower bound.
      - `end`: The other bound for the signal value
      - `delta`: The amount to add to the signal value at each iteration. If this operation would exceed the specified bounds, then the signal value saturates at the boundary value.

This mock supports [config overrides](../../docs/config-overrides.md). The override filename is `mock_digital_twin_config.json`, and the default config is located at `res/mock_digital_twin_config.default.json`.

## Behavior

The application maintains an internal count, and only entities satisfying the condition `begin <= count [< end]` will be enabled for all APIs. To increment this count and potentially change the set of enabled entities, press <kbd>Enter</kbd> in the application's console. This allows manual control over when the entities are turned on or off and permits straightforward mocking of more complex scenarios. As a result of this behavior, it is recommended to write configs such that a state change happens each time enter is pressed. For example, if a mock scenario has `n` different desired states, then all numbers in the range `0..n-1` should appear as values for at least one `begin` or `end` property. Otherwise pressing <kbd>Enter</kbd> will sometimes have no effect.

In addition, the mock also maintains a count of the number of times each provider has been invoked, and returns a value that is a function of this count. In this way, the behavior of the `generate_signal_value()` API is identical to that of the In-Memory Provider Proxy.

Entities that support the `Subscribe` operation will allow clients to send a request to the `/providers/subscribe/{provider_id}` endpoint, and the server will periodically publish the entity values to the provided callback. The communication protocol used by these mocked providers for this callback is HTTP.

Similarly, providers that support the `Get` operation will allow clients to send a request to the `/providers/get/{provider_id}` endpoint. The server will publish the entity values a single time to the provided callback rather than setting up a recurring callback. If the client wishes to retrieve the values again, then the client would need to send another request.
