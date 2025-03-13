| ⚠️ | `opamp-rs` is in preview and licensed under the New Relic Pre-Release Software Notice. |
|----|:----------------------------------------------------------------------------------------------------|

# `opamp-rs`: An OpAMP protocol client implementation in Rust

[![test](https://github.com/newrelic/opamp-rs/actions/workflows/test.yaml/badge.svg)](https://github.com/newrelic/opamp-rs/actions/workflows/test.yaml) [![coverage](https://github.com/newrelic/opamp-rs/actions/workflows/coverage.yaml/badge.svg)](https://github.com/newrelic/opamp-rs/actions/workflows/coverage.yaml) [[![docs](https://github.com/newrelic/newrelic-opamp-rs/actions/workflows/docs.yaml/badge.svg)](https://newrelic.github.io/newrelic-opamp-rs/)

[Open Agent Management Protocol (OpAMP)](https://github.com/open-telemetry/opamp-spec) is a network protocol for remote management of large fleets of data collection Agents.

OpAMP allows Agents to report their status to and receive configuration from a Server and to receive agent package updates from the server. The protocol is vendor-agnostic, so the Server can remotely monitor and manage a fleet of different Agents that implement OpAMP, including a fleet of mixed agents from different vendors.

This repository is an OpAMP client implementation in Rust, using HTTP as transport.

## Installation

The library is not available on [`crates.io`](https://crates.io/) for now, but you can still use it from this repository by adding the following line to your project's `Cargo.toml`:

```toml
[dependencies]
opamp-client = { git = "ssh://git@github.com/newrelic/newrelic-opamp-rs.git", tag = "0.0.30" }
```

## Getting Started

The library is designed to be generic over a provided HTTP client, such as `reqwest`. Make sure your client type implements the trait `HttpClient` and call `NotStartedHttpClient::new` with it along with an implementation of `AgentCallbacks` and your `StartSettings`. Use `with_interval` to modify the polling frequency.

After the above steps, calling `start` on the resulting value will start the server and return an owned value for the started server. This will be an implementation of `Client` with which you can call methods to perform the supported OpAMP operations according to the specification, such as `set_agent_description`, `set_health`, `set_remote_config_status` and so on.

Calling `stop` on this value will shut it down.

For more details, please check the documentation.

## Testing

The usual way of testing Rust crates is enough.

```sh
cargo test --locked --all-features --all-targets
```

## Support

If you find any problems while using the library or have a doubt, please feel free to open an [Issue](https://github.com/newrelic/opamp-rs/issues), where the New Relic maintainers of this project will be able to help.

## Contribute

We encourage your contributions to improve [project name]! Keep in mind that when you submit your pull request, you'll need to sign the CLA via the click-through using CLA-Assistant. You only have to sign the CLA one time per project.

If you have any questions, or to execute our corporate CLA (which is required if your contribution is on behalf of a company), drop us an email at <opensource@newrelic.com>.

### A note about vulnerabilities

As noted in our [security policy](../../security/policy), New Relic is committed to the privacy and security of our customers and their data. We believe that providing coordinated disclosure by security researchers and engaging with the security community are important means to achieve our security goals.

If you believe you have found a security vulnerability in this project or any of New Relic's products or websites, we welcome and greatly appreciate you reporting it to New Relic through [HackerOne](https://hackerone.com/newrelic).

If you would like to contribute to this project, review [these guidelines](./CONTRIBUTING.md).

To all contributors, we thank you! Without your contribution, this project would not be what it is today.

## License

`opamp-rs` is licensed under the New Relic Pre-Release Software Notice.

`opamp-rs` also uses source code from third-party libraries. You can find full details on which libraries are used and the terms under which they are licensed in the third-party notices document.

## Upstream archive

[Link](https://github.com/newrelic/opamp-rs) (private for NR employees).
