/// AnyValue is used to represent any type of attribute value. AnyValue may contain a
/// primitive value such as a string or integer or it may contain an arbitrary nested
/// object containing arrays, key-value lists and primitives.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnyValue {
    /// The value is one of the listed fields. It is valid for all values to be unspecified
    /// in which case this AnyValue is considered to be "null".
    #[prost(oneof = "any_value::Value", tags = "1, 2, 3, 4, 5, 6, 7")]
    pub value: ::core::option::Option<any_value::Value>,
}
/// Nested message and enum types in `AnyValue`.
pub mod any_value {
    /// The value is one of the listed fields. It is valid for all values to be unspecified
    /// in which case this AnyValue is considered to be "null".
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Value {
        #[prost(string, tag = "1")]
        StringValue(::prost::alloc::string::String),
        #[prost(bool, tag = "2")]
        BoolValue(bool),
        #[prost(int64, tag = "3")]
        IntValue(i64),
        #[prost(double, tag = "4")]
        DoubleValue(f64),
        #[prost(message, tag = "5")]
        ArrayValue(super::ArrayValue),
        #[prost(message, tag = "6")]
        KvlistValue(super::KeyValueList),
        #[prost(bytes, tag = "7")]
        BytesValue(::prost::alloc::vec::Vec<u8>),
    }
}
/// ArrayValue is a list of AnyValue messages. We need ArrayValue as a message
/// since oneof in AnyValue does not allow repeated fields.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArrayValue {
    /// Array of values. The array may be empty (contain 0 elements).
    #[prost(message, repeated, tag = "1")]
    pub values: ::prost::alloc::vec::Vec<AnyValue>,
}
/// KeyValueList is a list of KeyValue messages. We need KeyValueList as a message
/// since `oneof` in AnyValue does not allow repeated fields. Everywhere else where we need
/// a list of KeyValue messages (e.g. in Span) we use `repeated KeyValue` directly to
/// avoid unnecessary extra wrapping (which slows down the protocol). The 2 approaches
/// are semantically equivalent.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyValueList {
    /// A collection of key/value pairs of key-value pairs. The list may be empty (may
    /// contain 0 elements).
    #[prost(message, repeated, tag = "1")]
    pub values: ::prost::alloc::vec::Vec<KeyValue>,
}
/// KeyValue is a key-value pair that is used to store Span attributes, Link
/// attributes, etc.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KeyValue {
    #[prost(string, tag = "1")]
    pub key: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub value: ::core::option::Option<AnyValue>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AgentToServer {
    /// Globally unique identifier of the running instance of the Agent. SHOULD remain
    /// unchanged for the lifetime of the Agent process.
    /// MUST be 16 bytes long and SHOULD be generated using the UUID v7 spec.
    #[prost(bytes = "vec", tag = "1")]
    pub instance_uid: ::prost::alloc::vec::Vec<u8>,
    /// The sequence number is incremented by 1 for every AgentToServer sent
    /// by the Agent. This allows the Server to detect that it missed a message when
    /// it notices that the sequence_num is not exactly by 1 greater than the previously
    /// received one.
    #[prost(uint64, tag = "2")]
    pub sequence_num: u64,
    /// Data that describes the Agent, its type, where it runs, etc.
    /// May be omitted if nothing changed since last AgentToServer message.
    #[prost(message, optional, tag = "3")]
    pub agent_description: ::core::option::Option<AgentDescription>,
    /// Bitmask of flags defined by AgentCapabilities enum.
    /// All bits that are not defined in AgentCapabilities enum MUST be set to 0 by
    /// the Agent. This allows extending the protocol and the AgentCapabilities enum
    /// in the future such that old Agents automatically report that they don't
    /// support the new capability.
    /// This field MUST be always set.
    #[prost(uint64, tag = "4")]
    pub capabilities: u64,
    /// The current health of the Agent and sub-components. The top-level ComponentHealth represents
    /// the health of the Agent overall. May be omitted if nothing changed since last AgentToServer
    /// message.
    /// Status: \[Beta\]
    #[prost(message, optional, tag = "5")]
    pub health: ::core::option::Option<ComponentHealth>,
    /// The current effective configuration of the Agent. The effective configuration is
    /// the one that is currently used by the Agent. The effective configuration may be
    /// different from the remote configuration received from the Server earlier, e.g.
    /// because the Agent uses a local configuration instead (or in addition).
    ///
    /// This field SHOULD be unset if the effective config is unchanged since the last
    /// AgentToServer message.
    #[prost(message, optional, tag = "6")]
    pub effective_config: ::core::option::Option<EffectiveConfig>,
    /// The status of the remote config that was previously received from the Server.
    /// This field SHOULD be unset if the remote config status is unchanged since the
    /// last AgentToServer message.
    #[prost(message, optional, tag = "7")]
    pub remote_config_status: ::core::option::Option<RemoteConfigStatus>,
    /// The list of the Agent packages, including package statuses. This field SHOULD be
    /// unset if this information is unchanged since the last AgentToServer message for
    /// this Agent was sent in the stream.
    /// Status: \[Beta\]
    #[prost(message, optional, tag = "8")]
    pub package_statuses: ::core::option::Option<PackageStatuses>,
    /// AgentDisconnect MUST be set in the last AgentToServer message sent from the
    /// Agent to the Server.
    #[prost(message, optional, tag = "9")]
    pub agent_disconnect: ::core::option::Option<AgentDisconnect>,
    /// Bit flags as defined by AgentToServerFlags bit masks.
    #[prost(uint64, tag = "10")]
    pub flags: u64,
    /// A request to create connection settings. This field is set for flows where
    /// the Agent initiates the creation of connection settings.
    /// Status: \[Development\]
    #[prost(message, optional, tag = "11")]
    pub connection_settings_request: ::core::option::Option<ConnectionSettingsRequest>,
    /// A message indicating custom capabilities supported by the Agent.
    /// Status: \[Development\]
    #[prost(message, optional, tag = "12")]
    pub custom_capabilities: ::core::option::Option<CustomCapabilities>,
    /// A custom message sent from an Agent to the Server.
    /// Status: \[Development\]
    #[prost(message, optional, tag = "13")]
    pub custom_message: ::core::option::Option<CustomMessage>,
}
/// AgentDisconnect is the last message sent from the Agent to the Server. The Server
/// SHOULD forget the association of the Agent instance with the message stream.
///
/// If the message stream is closed in the transport layer then the Server SHOULD
/// forget association of all Agent instances that were previously established for
/// this message stream using AgentConnect message, even if the corresponding
/// AgentDisconnect message were not explicitly received from the Agent.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AgentDisconnect {}
/// ConnectionSettingsRequest is a request from the Agent to the Server to create
/// and respond with an offer of connection settings for the Agent.
/// Status: \[Development\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectionSettingsRequest {
    /// Request for OpAMP connection settings. If this field is unset
    /// then the ConnectionSettingsRequest message is empty and is not actionable
    /// for the Server.
    #[prost(message, optional, tag = "1")]
    pub opamp: ::core::option::Option<OpAmpConnectionSettingsRequest>,
}
/// OpAMPConnectionSettingsRequest is a request for the Server to produce
/// a OpAMPConnectionSettings in its response.
/// Status: \[Development\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OpAmpConnectionSettingsRequest {
    /// A request to create a client certificate. This is used to initiate a
    /// Client Signing Request (CSR) flow.
    /// Required.
    #[prost(message, optional, tag = "1")]
    pub certificate_request: ::core::option::Option<CertificateRequest>,
}
/// Status: \[Development\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CertificateRequest {
    /// PEM-encoded Client Certificate Signing Request (CSR), signed by client's private key.
    /// The Server SHOULD validate the request and SHOULD respond with a
    /// OpAMPConnectionSettings where the certificate.public_key contains the issued
    /// certificate.
    #[prost(bytes = "vec", tag = "1")]
    pub csr: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerToAgent {
    /// Agent instance uid. MUST match the instance_uid field in AgentToServer message.
    /// Used for multiplexing messages from/to multiple agents using one message stream.
    #[prost(bytes = "vec", tag = "1")]
    pub instance_uid: ::prost::alloc::vec::Vec<u8>,
    /// error_response is set if the Server wants to indicate that something went wrong
    /// during processing of an AgentToServer message. If error_response is set then
    /// all other fields below must be unset and vice versa, if any of the fields below is
    /// set then error_response must be unset.
    #[prost(message, optional, tag = "2")]
    pub error_response: ::core::option::Option<ServerErrorResponse>,
    /// remote_config field is set when the Server has a remote config offer for the Agent.
    #[prost(message, optional, tag = "3")]
    pub remote_config: ::core::option::Option<AgentRemoteConfig>,
    /// This field is set when the Server wants the Agent to change one or more
    /// of its client connection settings (destination, headers, certificate, etc).
    /// Status: \[Beta\]
    #[prost(message, optional, tag = "4")]
    pub connection_settings: ::core::option::Option<ConnectionSettingsOffers>,
    /// This field is set when the Server has packages to offer to the Agent.
    /// Status: \[Beta\]
    #[prost(message, optional, tag = "5")]
    pub packages_available: ::core::option::Option<PackagesAvailable>,
    /// Bit flags as defined by ServerToAgentFlags bit masks.
    #[prost(uint64, tag = "6")]
    pub flags: u64,
    /// Bitmask of flags defined by ServerCapabilities enum.
    /// All bits that are not defined in ServerCapabilities enum MUST be set to 0
    /// by the Server. This allows extending the protocol and the ServerCapabilities
    /// enum in the future such that old Servers automatically report that they
    /// don't support the new capability.
    /// This field MUST be set in the first ServerToAgent sent by the Server and MAY
    /// be omitted in subsequent ServerToAgent messages by setting it to
    /// UnspecifiedServerCapability value.
    #[prost(uint64, tag = "7")]
    pub capabilities: u64,
    /// Properties related to identification of the Agent, which can be overridden
    /// by the Server if needed.
    #[prost(message, optional, tag = "8")]
    pub agent_identification: ::core::option::Option<AgentIdentification>,
    /// Allows the Server to instruct the Agent to perform a command, e.g. RESTART. This field should not be specified
    /// with fields other than instance_uid and capabilities. If specified, other fields will be ignored and the command
    /// will be performed.
    /// Status: \[Beta\]
    #[prost(message, optional, tag = "9")]
    pub command: ::core::option::Option<ServerToAgentCommand>,
    /// A message indicating custom capabilities supported by the Server.
    /// Status: \[Development\]
    #[prost(message, optional, tag = "10")]
    pub custom_capabilities: ::core::option::Option<CustomCapabilities>,
    /// A custom message sent from the Server to an Agent.
    /// Status: \[Development\]
    #[prost(message, optional, tag = "11")]
    pub custom_message: ::core::option::Option<CustomMessage>,
}
/// The OpAMPConnectionSettings message is a collection of fields which comprise an
/// offer from the Server to the Agent to use the specified settings for OpAMP
/// connection.
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OpAmpConnectionSettings {
    /// OpAMP Server URL This MUST be a WebSocket or HTTP URL and MUST be non-empty, for
    /// example: "wss://example.com:4318/v1/opamp"
    #[prost(string, tag = "1")]
    pub destination_endpoint: ::prost::alloc::string::String,
    /// Optional headers to use when connecting. Typically used to set access tokens or
    /// other authorization headers. For HTTP-based protocols the Agent should
    /// set these in the request headers.
    /// For example:
    /// key="Authorization", Value="Basic YWxhZGRpbjpvcGVuc2VzYW1l".
    #[prost(message, optional, tag = "2")]
    pub headers: ::core::option::Option<Headers>,
    /// The Agent should use the offered certificate to connect to the destination
    /// from now on. If the Agent is able to validate and connect using the offered
    /// certificate the Agent SHOULD forget any previous client certificates
    /// for this connection.
    /// This field is optional: if omitted the client SHOULD NOT use a client-side certificate.
    /// This field can be used to perform a client certificate revocation/rotation.
    #[prost(message, optional, tag = "3")]
    pub certificate: ::core::option::Option<TlsCertificate>,
}
/// The TelemetryConnectionSettings message is a collection of fields which comprise an
/// offer from the Server to the Agent to use the specified settings for a network
/// connection to report own telemetry.
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TelemetryConnectionSettings {
    /// The value MUST be a full URL an OTLP/HTTP/Protobuf receiver with path. Schema
    /// SHOULD begin with "<https://",> for example "<https://example.com:4318/v1/metrics">
    /// The Agent MAY refuse to send the telemetry if the URL begins with "<http://".>
    #[prost(string, tag = "1")]
    pub destination_endpoint: ::prost::alloc::string::String,
    /// Optional headers to use when connecting. Typically used to set access tokens or
    /// other authorization headers. For HTTP-based protocols the Agent should
    /// set these in the request headers.
    /// For example:
    /// key="Authorization", Value="Basic YWxhZGRpbjpvcGVuc2VzYW1l".
    #[prost(message, optional, tag = "2")]
    pub headers: ::core::option::Option<Headers>,
    /// The Agent should use the offered certificate to connect to the destination
    /// from now on. If the Agent is able to validate and connect using the offered
    /// certificate the Agent SHOULD forget any previous client certificates
    /// for this connection.
    /// This field is optional: if omitted the client SHOULD NOT use a client-side certificate.
    /// This field can be used to perform a client certificate revocation/rotation.
    #[prost(message, optional, tag = "3")]
    pub certificate: ::core::option::Option<TlsCertificate>,
}
/// The OtherConnectionSettings message is a collection of fields which comprise an
/// offer from the Server to the Agent to use the specified settings for a network
/// connection. It is not required that all fields in this message are specified.
/// The Server may specify only some of the fields, in which case it means that
/// the Server offers the Agent to change only those fields, while keeping the
/// rest of the fields unchanged.
///
/// For example the Server may send a ConnectionSettings message with only the
/// certificate field set, while all other fields are unset. This means that
/// the Server wants the Agent to use a new certificate and continue sending to
/// the destination it is currently sending using the current header and other
/// settings.
///
/// For fields which reference other messages the field is considered unset
/// when the reference is unset.
///
/// For primitive field (string) we rely on the "flags" to describe that the
/// field is not set (this is done to overcome the limitation of old protoc
/// compilers don't generate methods that allow to check for the presence of
/// the field.
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OtherConnectionSettings {
    /// A URL, host:port or some other destination specifier.
    #[prost(string, tag = "1")]
    pub destination_endpoint: ::prost::alloc::string::String,
    /// Optional headers to use when connecting. Typically used to set access tokens or
    /// other authorization headers. For HTTP-based protocols the Agent should
    /// set these in the request headers.
    /// For example:
    /// key="Authorization", Value="Basic YWxhZGRpbjpvcGVuc2VzYW1l".
    #[prost(message, optional, tag = "2")]
    pub headers: ::core::option::Option<Headers>,
    /// The Agent should use the offered certificate to connect to the destination
    /// from now on. If the Agent is able to validate and connect using the offered
    /// certificate the Agent SHOULD forget any previous client certificates
    /// for this connection.
    /// This field is optional: if omitted the client SHOULD NOT use a client-side certificate.
    /// This field can be used to perform a client certificate revocation/rotation.
    #[prost(message, optional, tag = "3")]
    pub certificate: ::core::option::Option<TlsCertificate>,
    /// Other connection settings. These are Agent-specific and are up to the Agent
    /// interpret.
    #[prost(map = "string, string", tag = "4")]
    pub other_settings: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Headers {
    #[prost(message, repeated, tag = "1")]
    pub headers: ::prost::alloc::vec::Vec<Header>,
}
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Header {
    #[prost(string, tag = "1")]
    pub key: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub value: ::prost::alloc::string::String,
}
/// Status: \[Beta\]
///
/// The (public_key,private_key) certificate pair should be issued and
/// signed by a Certificate Authority that the destination Server recognizes.
///
/// It is highly recommended that the private key of the CA certificate is NOT
/// stored on the destination Server otherwise compromising the Server will allow
/// a malicious actor to issue valid Server certificates which will be automatically
/// trusted by all agents and will allow the actor to trivially MITM Agent-to-Server
/// traffic of all servers that use this CA certificate for their Server-side
/// certificates.
///
/// Alternatively the certificate may be self-signed, assuming the Server can
/// verify the certificate.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TlsCertificate {
    /// PEM-encoded public key of the certificate. Required.
    #[prost(bytes = "vec", tag = "1")]
    pub public_key: ::prost::alloc::vec::Vec<u8>,
    /// PEM-encoded private key of the certificate. Required.
    #[prost(bytes = "vec", tag = "2")]
    pub private_key: ::prost::alloc::vec::Vec<u8>,
    /// PEM-encoded public key of the CA that signed this certificate.
    /// Optional. MUST be specified if the certificate is CA-signed.
    /// Can be stored by TLS-terminating intermediary proxies in order to verify
    /// the connecting client's certificate in the future.
    /// It is not recommended that the Agent accepts this CA as an authority for
    /// any purposes.
    #[prost(bytes = "vec", tag = "3")]
    pub ca_public_key: ::prost::alloc::vec::Vec<u8>,
}
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectionSettingsOffers {
    /// Hash of all settings, including settings that may be omitted from this message
    /// because they are unchanged.
    #[prost(bytes = "vec", tag = "1")]
    pub hash: ::prost::alloc::vec::Vec<u8>,
    /// Settings to connect to the OpAMP Server.
    /// If this field is not set then the Agent should assume that the settings are
    /// unchanged and should continue using existing settings.
    /// The Agent MUST verify the offered connection settings by actually connecting
    /// before accepting the setting to ensure it does not loose access to the OpAMP
    /// Server due to invalid settings.
    #[prost(message, optional, tag = "2")]
    pub opamp: ::core::option::Option<OpAmpConnectionSettings>,
    /// Settings to connect to an OTLP metrics backend to send Agent's own metrics to.
    /// If this field is not set then the Agent should assume that the settings
    /// are unchanged.
    ///
    /// Once accepted the Agent should periodically send to the specified destination
    /// its own metrics, i.e. metrics of the Agent process and any custom metrics that
    /// describe the Agent state.
    ///
    /// All attributes specified in the identifying_attributes field in AgentDescription
    /// message SHOULD be also specified in the Resource of the reported OTLP metrics.
    ///
    /// Attributes specified in the non_identifying_attributes field in
    /// AgentDescription message may be also specified in the Resource of the reported
    /// OTLP metrics, in which case they SHOULD have exactly the same values.
    ///
    /// Process metrics MUST follow the conventions for processes:
    /// <https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/semantic_conventions/process-metrics.md>
    #[prost(message, optional, tag = "3")]
    pub own_metrics: ::core::option::Option<TelemetryConnectionSettings>,
    /// Similar to own_metrics, but for traces.
    #[prost(message, optional, tag = "4")]
    pub own_traces: ::core::option::Option<TelemetryConnectionSettings>,
    /// Similar to own_metrics, but for logs.
    #[prost(message, optional, tag = "5")]
    pub own_logs: ::core::option::Option<TelemetryConnectionSettings>,
    /// Another set of connection settings, with a string name associated with each.
    /// How the Agent uses these is Agent-specific. Typically the name represents
    /// the name of the destination to connect to (as it is known to the Agent).
    /// If this field is not set then the Agent should assume that the other_connections
    /// settings are unchanged.
    #[prost(map = "string, message", tag = "6")]
    pub other_connections: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        OtherConnectionSettings,
    >,
}
/// List of packages that the Server offers to the Agent.
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PackagesAvailable {
    /// Map of packages. Keys are package names, values are the packages available for download.
    #[prost(map = "string, message", tag = "1")]
    pub packages: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        PackageAvailable,
    >,
    /// Aggregate hash of all remotely installed packages. The Agent SHOULD include this
    /// value in subsequent PackageStatuses messages. This in turn allows the management
    /// Server to identify that a different set of packages is available for the Agent
    /// and specify the available packages in the next ServerToAgent message.
    ///
    /// This field MUST be always set if the management Server supports packages
    /// of agents.
    ///
    /// The hash is calculated as an aggregate of all packages names and content.
    #[prost(bytes = "vec", tag = "2")]
    pub all_packages_hash: ::prost::alloc::vec::Vec<u8>,
}
/// Each Agent is composed of one or more packages. A package has a name and
/// content stored in a file. The content of the files, functionality
/// provided by the packages, how they are stored and used by the Agent side is Agent
/// type-specific and is outside the concerns of the OpAMP protocol.
///
/// If the Agent does not have an installed package with the specified name then
/// it SHOULD download it from the specified URL and install it.
///
/// If the Agent already has an installed package with the specified name
/// but with a different hash then the Agent SHOULD download and
/// install the package again, since it is a different version of the same package.
///
/// If the Agent has an installed package with the specified name and the same
/// hash then the Agent does not need to do anything, it already
/// has the right version of the package.
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PackageAvailable {
    #[prost(enumeration = "PackageType", tag = "1")]
    pub r#type: i32,
    /// The package version that is available on the Server side. The Agent may for
    /// example use this information to avoid downloading a package that was previously
    /// already downloaded and failed to install.
    #[prost(string, tag = "2")]
    pub version: ::prost::alloc::string::String,
    /// The downloadable file of the package.
    #[prost(message, optional, tag = "3")]
    pub file: ::core::option::Option<DownloadableFile>,
    /// The hash of the package. SHOULD be calculated based on all other fields of the
    /// PackageAvailable message and content of the file of the package. The hash is
    /// used by the Agent to determine if the package it has is different from the
    /// package the Server is offering.
    #[prost(bytes = "vec", tag = "4")]
    pub hash: ::prost::alloc::vec::Vec<u8>,
}
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DownloadableFile {
    /// The URL from which the file can be downloaded using HTTP GET request.
    /// The Server at the specified URL SHOULD support range requests
    /// to allow for resuming downloads.
    #[prost(string, tag = "1")]
    pub download_url: ::prost::alloc::string::String,
    /// The hash of the file content. Can be used by the Agent to verify that the file
    /// was downloaded correctly.
    #[prost(bytes = "vec", tag = "2")]
    pub content_hash: ::prost::alloc::vec::Vec<u8>,
    /// Optional signature of the file content. Can be used by the Agent to verify the
    /// authenticity of the downloaded file, for example can be the
    /// [detached GPG signature](<https://www.gnupg.org/gph/en/manual/x135.html#AEN160>).
    /// The exact signing and verification method is Agent specific. See
    /// <https://github.com/open-telemetry/opamp-spec/blob/main/specification.md#code-signing>
    /// for recommendations.
    #[prost(bytes = "vec", tag = "3")]
    pub signature: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerErrorResponse {
    #[prost(enumeration = "ServerErrorResponseType", tag = "1")]
    pub r#type: i32,
    /// Error message in the string form, typically human readable.
    #[prost(string, tag = "2")]
    pub error_message: ::prost::alloc::string::String,
    #[prost(oneof = "server_error_response::Details", tags = "3")]
    pub details: ::core::option::Option<server_error_response::Details>,
}
/// Nested message and enum types in `ServerErrorResponse`.
pub mod server_error_response {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Details {
        /// Additional information about retrying if type==UNAVAILABLE.
        #[prost(message, tag = "3")]
        RetryInfo(super::RetryInfo),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RetryInfo {
    #[prost(uint64, tag = "1")]
    pub retry_after_nanoseconds: u64,
}
/// ServerToAgentCommand is sent from the Server to the Agent to request that the Agent
/// perform a command.
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerToAgentCommand {
    #[prost(enumeration = "CommandType", tag = "1")]
    pub r#type: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AgentDescription {
    /// Attributes that identify the Agent.
    /// Keys/values are according to OpenTelemetry semantic conventions, see:
    /// <https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/resource/semantic_conventions>
    ///
    /// For standalone running Agents (such as OpenTelemetry Collector) the following
    /// attributes SHOULD be specified:
    /// - service.name should be set to a reverse FQDN that uniquely identifies the
    ///    Agent type, e.g. "io.opentelemetry.collector"
    /// - service.namespace if it is used in the environment where the Agent runs.
    /// - service.version should be set to version number of the Agent build.
    /// - service.instance.id should be set. It may be be set equal to the Agent's
    ///    instance uid (equal to ServerToAgent.instance_uid field) or any other value
    ///    that uniquely identifies the Agent in combination with other attributes.
    /// - any other attributes that are necessary for uniquely identifying the Agent's
    ///    own telemetry.
    ///
    /// The Agent SHOULD also include these attributes in the Resource of its own
    /// telemetry. The combination of identifying attributes SHOULD be sufficient to
    /// uniquely identify the Agent's own telemetry in the destination system to which
    /// the Agent sends its own telemetry.
    #[prost(message, repeated, tag = "1")]
    pub identifying_attributes: ::prost::alloc::vec::Vec<KeyValue>,
    /// Attributes that do not necessarily identify the Agent but help describe
    /// where it runs.
    /// The following attributes SHOULD be included:
    /// - os.type, os.version - to describe where the Agent runs.
    /// - host.* to describe the host the Agent runs on.
    /// - cloud.* to describe the cloud where the host is located.
    /// - any other relevant Resource attributes that describe this Agent and the
    ///    environment it runs in.
    /// - any user-defined attributes that the end user would like to associate
    ///    with this Agent.
    #[prost(message, repeated, tag = "2")]
    pub non_identifying_attributes: ::prost::alloc::vec::Vec<KeyValue>,
}
/// The health of the Agent and sub-components
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ComponentHealth {
    /// Set to true if the component is up and healthy.
    #[prost(bool, tag = "1")]
    pub healthy: bool,
    /// Timestamp since the component is up, i.e. when the component was started.
    /// Value is UNIX Epoch time in nanoseconds since 00:00:00 UTC on 1 January 1970.
    /// If the component is not running MUST be set to 0.
    #[prost(fixed64, tag = "2")]
    pub start_time_unix_nano: u64,
    /// Human-readable error message if the component is in erroneous state. SHOULD be set
    /// when healthy==false.
    #[prost(string, tag = "3")]
    pub last_error: ::prost::alloc::string::String,
    /// Component status represented as a string. The status values are defined by agent-specific
    /// semantics and not at the protocol level.
    #[prost(string, tag = "4")]
    pub status: ::prost::alloc::string::String,
    /// The time when the component status was observed. Value is UNIX Epoch time in
    /// nanoseconds since 00:00:00 UTC on 1 January 1970.
    #[prost(fixed64, tag = "5")]
    pub status_time_unix_nano: u64,
    /// A map to store more granular, sub-component health. It can nest as deeply as needed to
    /// describe the underlying system.
    #[prost(map = "string, message", tag = "6")]
    pub component_health_map: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ComponentHealth,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EffectiveConfig {
    /// The effective config of the Agent.
    #[prost(message, optional, tag = "1")]
    pub config_map: ::core::option::Option<AgentConfigMap>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoteConfigStatus {
    /// The hash of the remote config that was last received by this Agent in the
    /// AgentRemoteConfig.config_hash field.
    /// The Server SHOULD compare this hash with the config hash
    /// it has for the Agent and if the hashes are different the Server MUST include
    /// the remote_config field in the response in the ServerToAgent message.
    #[prost(bytes = "vec", tag = "1")]
    pub last_remote_config_hash: ::prost::alloc::vec::Vec<u8>,
    #[prost(enumeration = "RemoteConfigStatuses", tag = "2")]
    pub status: i32,
    /// Optional error message if status==FAILED.
    #[prost(string, tag = "3")]
    pub error_message: ::prost::alloc::string::String,
}
/// The PackageStatuses message describes the status of all packages that the Agent
/// has or was offered.
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PackageStatuses {
    /// A map of PackageStatus messages, where the keys are package names.
    /// The key MUST match the name field of PackageStatus message.
    #[prost(map = "string, message", tag = "1")]
    pub packages: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        PackageStatus,
    >,
    /// The aggregate hash of all packages that this Agent previously received from the
    /// Server via PackagesAvailable message.
    ///
    /// The Server SHOULD compare this hash to the aggregate hash of all packages that
    /// it has for this Agent and if the hashes are different the Server SHOULD send
    /// an PackagesAvailable message to the Agent.
    #[prost(bytes = "vec", tag = "2")]
    pub server_provided_all_packages_hash: ::prost::alloc::vec::Vec<u8>,
    /// This field is set if the Agent encountered an error when processing the
    /// PackagesAvailable message and that error is not related to any particular single
    /// package.
    /// The field must be unset is there were no processing errors.
    #[prost(string, tag = "3")]
    pub error_message: ::prost::alloc::string::String,
}
/// The status of a single package.
/// Status: \[Beta\]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PackageStatus {
    /// Package name. MUST be always set and MUST match the key in the packages field
    /// of PackageStatuses message.
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// The version of the package that the Agent has.
    /// MUST be set if the Agent has this package.
    /// MUST be empty if the Agent does not have this package. This may be the case
    /// for example if the package was offered by the Server but failed to install
    /// and the Agent did not have this package previously.
    #[prost(string, tag = "2")]
    pub agent_has_version: ::prost::alloc::string::String,
    /// The hash of the package that the Agent has.
    /// MUST be set if the Agent has this package.
    /// MUST be empty if the Agent does not have this package. This may be the case for
    /// example if the package was offered by the Server but failed to install and the
    /// Agent did not have this package previously.
    #[prost(bytes = "vec", tag = "3")]
    pub agent_has_hash: ::prost::alloc::vec::Vec<u8>,
    /// The version of the package that the Server offered to the Agent.
    /// MUST be set if the installation of the package is initiated by an earlier offer
    /// from the Server to install this package.
    ///
    /// MUST be empty if the Agent has this package but it was installed locally and
    /// was not offered by the Server.
    ///
    /// Note that it is possible for both agent_has_version and server_offered_version
    /// fields to be set and to have different values. This is for example possible if
    /// the Agent already has a version of the package successfully installed, the Server
    /// offers a different version, but the Agent fails to install that version.
    #[prost(string, tag = "4")]
    pub server_offered_version: ::prost::alloc::string::String,
    /// The hash of the package that the Server offered to the Agent.
    /// MUST be set if the installation of the package is initiated by an earlier
    /// offer from the Server to install this package.
    ///
    /// MUST be empty if the Agent has this package but it was installed locally and
    /// was not offered by the Server.
    ///
    /// Note that it is possible for both agent_has_hash and server_offered_hash
    /// fields to be set and to have different values. This is for example possible if
    /// the Agent already has a version of the package successfully installed, the
    /// Server offers a different version, but the Agent fails to install that version.
    #[prost(bytes = "vec", tag = "5")]
    pub server_offered_hash: ::prost::alloc::vec::Vec<u8>,
    #[prost(enumeration = "PackageStatusEnum", tag = "6")]
    pub status: i32,
    /// Error message if the status is erroneous.
    #[prost(string, tag = "7")]
    pub error_message: ::prost::alloc::string::String,
}
/// Properties related to identification of the Agent, which can be overridden
/// by the Server if needed
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AgentIdentification {
    /// When new_instance_uid is set, Agent MUST update instance_uid
    /// to the value provided and use it for all further communication.
    /// MUST be 16 bytes long and SHOULD be generated using the UUID v7 spec.
    #[prost(bytes = "vec", tag = "1")]
    pub new_instance_uid: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AgentRemoteConfig {
    /// Agent config offered by the management Server to the Agent instance. SHOULD NOT be
    /// set if the config for this Agent has not changed since it was last requested (i.e.
    /// AgentConfigRequest.last_remote_config_hash field is equal to
    /// AgentConfigResponse.config_hash field).
    #[prost(message, optional, tag = "1")]
    pub config: ::core::option::Option<AgentConfigMap>,
    /// Hash of "config". The Agent SHOULD include this value in subsequent
    /// RemoteConfigStatus messages in the last_remote_config_hash field. This in turn
    /// allows the management Server to identify that a new config is available for the Agent.
    ///
    /// This field MUST be always set if the management Server supports remote configuration
    /// of agents.
    ///
    /// Management Server must choose a hashing function that guarantees lack of hash
    /// collisions in practice.
    #[prost(bytes = "vec", tag = "2")]
    pub config_hash: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AgentConfigMap {
    /// Map of configs. Keys are config file names or config section names.
    /// The configuration is assumed to be a collection of one or more named config files
    /// or sections.
    /// For agents that use a single config file or section the map SHOULD contain a single
    /// entry and the key may be an empty string.
    #[prost(map = "string, message", tag = "1")]
    pub config_map: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        AgentConfigFile,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AgentConfigFile {
    /// Config file or section body. The content, format and encoding depends on the Agent
    /// type. The content_type field may optionally describe the MIME type of the body.
    #[prost(bytes = "vec", tag = "1")]
    pub body: ::prost::alloc::vec::Vec<u8>,
    /// Optional MIME Content-Type that describes what's in the body field, for
    /// example "text/yaml".
    #[prost(string, tag = "2")]
    pub content_type: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CustomCapabilities {
    /// A list of custom capabilities that are supported. Each capability is a reverse FQDN
    /// with optional version information that uniquely identifies the custom capability
    /// and should match a capability specified in a supported CustomMessage.
    /// Status: \[Development\]
    #[prost(string, repeated, tag = "1")]
    pub capabilities: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CustomMessage {
    /// A reverse FQDN that uniquely identifies the capability and matches one of the
    /// capabilities in the CustomCapabilities message.
    /// Status: \[Development\]
    #[prost(string, tag = "1")]
    pub capability: ::prost::alloc::string::String,
    /// Type of message within the capability. The capability defines the types of custom
    /// messages that are used to implement the capability. The type must only be unique
    /// within the capability.
    /// Status: \[Development\]
    #[prost(string, tag = "2")]
    pub r#type: ::prost::alloc::string::String,
    /// Binary data of the message. The capability must specify the format of the contents
    /// of the data for each custom message type it defines.
    /// Status: \[Development\]
    #[prost(bytes = "vec", tag = "3")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AgentToServerFlags {
    Unspecified = 0,
    /// The Agent requests Server go generate a new instance_uid, which will
    /// be sent back in ServerToAgent message
    RequestInstanceUid = 1,
}
impl AgentToServerFlags {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            AgentToServerFlags::Unspecified => "AgentToServerFlags_Unspecified",
            AgentToServerFlags::RequestInstanceUid => {
                "AgentToServerFlags_RequestInstanceUid"
            }
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "AgentToServerFlags_Unspecified" => Some(Self::Unspecified),
            "AgentToServerFlags_RequestInstanceUid" => Some(Self::RequestInstanceUid),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ServerToAgentFlags {
    Unspecified = 0,
    /// ReportFullState flag can be used by the Server if the Agent did not include the
    /// particular bit of information in the last status report (which is an allowed
    /// optimization) but the Server detects that it does not have it (e.g. was
    /// restarted and lost state). The detection happens using
    /// AgentToServer.sequence_num values.
    /// The Server asks the Agent to report full status.
    ReportFullState = 1,
}
impl ServerToAgentFlags {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ServerToAgentFlags::Unspecified => "ServerToAgentFlags_Unspecified",
            ServerToAgentFlags::ReportFullState => "ServerToAgentFlags_ReportFullState",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ServerToAgentFlags_Unspecified" => Some(Self::Unspecified),
            "ServerToAgentFlags_ReportFullState" => Some(Self::ReportFullState),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ServerCapabilities {
    /// The capabilities field is unspecified.
    Unspecified = 0,
    /// The Server can accept status reports. This bit MUST be set, since all Server
    /// MUST be able to accept status reports.
    AcceptsStatus = 1,
    /// The Server can offer remote configuration to the Agent.
    OffersRemoteConfig = 2,
    /// The Server can accept EffectiveConfig in AgentToServer.
    AcceptsEffectiveConfig = 4,
    /// The Server can offer Packages.
    /// Status: \[Beta\]
    OffersPackages = 8,
    /// The Server can accept Packages status.
    /// Status: \[Beta\]
    AcceptsPackagesStatus = 16,
    /// The Server can offer connection settings.
    /// Status: \[Beta\]
    OffersConnectionSettings = 32,
    /// The Server can accept ConnectionSettingsRequest and respond with an offer.
    /// Status: \[Development\]
    AcceptsConnectionSettingsRequest = 64,
}
impl ServerCapabilities {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ServerCapabilities::Unspecified => "ServerCapabilities_Unspecified",
            ServerCapabilities::AcceptsStatus => "ServerCapabilities_AcceptsStatus",
            ServerCapabilities::OffersRemoteConfig => {
                "ServerCapabilities_OffersRemoteConfig"
            }
            ServerCapabilities::AcceptsEffectiveConfig => {
                "ServerCapabilities_AcceptsEffectiveConfig"
            }
            ServerCapabilities::OffersPackages => "ServerCapabilities_OffersPackages",
            ServerCapabilities::AcceptsPackagesStatus => {
                "ServerCapabilities_AcceptsPackagesStatus"
            }
            ServerCapabilities::OffersConnectionSettings => {
                "ServerCapabilities_OffersConnectionSettings"
            }
            ServerCapabilities::AcceptsConnectionSettingsRequest => {
                "ServerCapabilities_AcceptsConnectionSettingsRequest"
            }
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ServerCapabilities_Unspecified" => Some(Self::Unspecified),
            "ServerCapabilities_AcceptsStatus" => Some(Self::AcceptsStatus),
            "ServerCapabilities_OffersRemoteConfig" => Some(Self::OffersRemoteConfig),
            "ServerCapabilities_AcceptsEffectiveConfig" => {
                Some(Self::AcceptsEffectiveConfig)
            }
            "ServerCapabilities_OffersPackages" => Some(Self::OffersPackages),
            "ServerCapabilities_AcceptsPackagesStatus" => {
                Some(Self::AcceptsPackagesStatus)
            }
            "ServerCapabilities_OffersConnectionSettings" => {
                Some(Self::OffersConnectionSettings)
            }
            "ServerCapabilities_AcceptsConnectionSettingsRequest" => {
                Some(Self::AcceptsConnectionSettingsRequest)
            }
            _ => None,
        }
    }
}
/// The type of the package, either an addon or a top-level package.
/// Status: \[Beta\]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PackageType {
    TopLevel = 0,
    Addon = 1,
}
impl PackageType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            PackageType::TopLevel => "PackageType_TopLevel",
            PackageType::Addon => "PackageType_Addon",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "PackageType_TopLevel" => Some(Self::TopLevel),
            "PackageType_Addon" => Some(Self::Addon),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ServerErrorResponseType {
    /// Unknown error. Something went wrong, but it is not known what exactly.
    /// The Agent SHOULD NOT retry the message.
    /// The error_message field may contain a description of the problem.
    Unknown = 0,
    /// The AgentToServer message was malformed. The Agent SHOULD NOT retry
    /// the message.
    BadRequest = 1,
    /// The Server is overloaded and unable to process the request. The Agent
    /// should retry the message later. retry_info field may be optionally
    /// set with additional information about retrying.
    Unavailable = 2,
}
impl ServerErrorResponseType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ServerErrorResponseType::Unknown => "ServerErrorResponseType_Unknown",
            ServerErrorResponseType::BadRequest => "ServerErrorResponseType_BadRequest",
            ServerErrorResponseType::Unavailable => "ServerErrorResponseType_Unavailable",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ServerErrorResponseType_Unknown" => Some(Self::Unknown),
            "ServerErrorResponseType_BadRequest" => Some(Self::BadRequest),
            "ServerErrorResponseType_Unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }
}
/// Status: \[Beta\]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum CommandType {
    /// The Agent should restart. This request will be ignored if the Agent does not
    /// support restart.
    Restart = 0,
}
impl CommandType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            CommandType::Restart => "CommandType_Restart",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "CommandType_Restart" => Some(Self::Restart),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AgentCapabilities {
    /// The capabilities field is unspecified.
    Unspecified = 0,
    /// The Agent can report status. This bit MUST be set, since all Agents MUST
    /// report status.
    ReportsStatus = 1,
    /// The Agent can accept remote configuration from the Server.
    AcceptsRemoteConfig = 2,
    /// The Agent will report EffectiveConfig in AgentToServer.
    ReportsEffectiveConfig = 4,
    /// The Agent can accept package offers.
    /// Status: \[Beta\]
    AcceptsPackages = 8,
    /// The Agent can report package status.
    /// Status: \[Beta\]
    ReportsPackageStatuses = 16,
    /// The Agent can report own trace to the destination specified by
    /// the Server via ConnectionSettingsOffers.own_traces field.
    /// Status: \[Beta\]
    ReportsOwnTraces = 32,
    /// The Agent can report own metrics to the destination specified by
    /// the Server via ConnectionSettingsOffers.own_metrics field.
    /// Status: \[Beta\]
    ReportsOwnMetrics = 64,
    /// The Agent can report own logs to the destination specified by
    /// the Server via ConnectionSettingsOffers.own_logs field.
    /// Status: \[Beta\]
    ReportsOwnLogs = 128,
    /// The can accept connections settings for OpAMP via
    /// ConnectionSettingsOffers.opamp field.
    /// Status: \[Beta\]
    AcceptsOpAmpConnectionSettings = 256,
    /// The can accept connections settings for other destinations via
    /// ConnectionSettingsOffers.other_connections field.
    /// Status: \[Beta\]
    AcceptsOtherConnectionSettings = 512,
    /// The Agent can accept restart requests.
    /// Status: \[Beta\]
    AcceptsRestartCommand = 1024,
    /// The Agent will report Health via AgentToServer.health field.
    ReportsHealth = 2048,
    /// The Agent will report RemoteConfig status via AgentToServer.remote_config_status field.
    ReportsRemoteConfig = 4096,
}
impl AgentCapabilities {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            AgentCapabilities::Unspecified => "AgentCapabilities_Unspecified",
            AgentCapabilities::ReportsStatus => "AgentCapabilities_ReportsStatus",
            AgentCapabilities::AcceptsRemoteConfig => {
                "AgentCapabilities_AcceptsRemoteConfig"
            }
            AgentCapabilities::ReportsEffectiveConfig => {
                "AgentCapabilities_ReportsEffectiveConfig"
            }
            AgentCapabilities::AcceptsPackages => "AgentCapabilities_AcceptsPackages",
            AgentCapabilities::ReportsPackageStatuses => {
                "AgentCapabilities_ReportsPackageStatuses"
            }
            AgentCapabilities::ReportsOwnTraces => "AgentCapabilities_ReportsOwnTraces",
            AgentCapabilities::ReportsOwnMetrics => "AgentCapabilities_ReportsOwnMetrics",
            AgentCapabilities::ReportsOwnLogs => "AgentCapabilities_ReportsOwnLogs",
            AgentCapabilities::AcceptsOpAmpConnectionSettings => {
                "AgentCapabilities_AcceptsOpAMPConnectionSettings"
            }
            AgentCapabilities::AcceptsOtherConnectionSettings => {
                "AgentCapabilities_AcceptsOtherConnectionSettings"
            }
            AgentCapabilities::AcceptsRestartCommand => {
                "AgentCapabilities_AcceptsRestartCommand"
            }
            AgentCapabilities::ReportsHealth => "AgentCapabilities_ReportsHealth",
            AgentCapabilities::ReportsRemoteConfig => {
                "AgentCapabilities_ReportsRemoteConfig"
            }
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "AgentCapabilities_Unspecified" => Some(Self::Unspecified),
            "AgentCapabilities_ReportsStatus" => Some(Self::ReportsStatus),
            "AgentCapabilities_AcceptsRemoteConfig" => Some(Self::AcceptsRemoteConfig),
            "AgentCapabilities_ReportsEffectiveConfig" => {
                Some(Self::ReportsEffectiveConfig)
            }
            "AgentCapabilities_AcceptsPackages" => Some(Self::AcceptsPackages),
            "AgentCapabilities_ReportsPackageStatuses" => {
                Some(Self::ReportsPackageStatuses)
            }
            "AgentCapabilities_ReportsOwnTraces" => Some(Self::ReportsOwnTraces),
            "AgentCapabilities_ReportsOwnMetrics" => Some(Self::ReportsOwnMetrics),
            "AgentCapabilities_ReportsOwnLogs" => Some(Self::ReportsOwnLogs),
            "AgentCapabilities_AcceptsOpAMPConnectionSettings" => {
                Some(Self::AcceptsOpAmpConnectionSettings)
            }
            "AgentCapabilities_AcceptsOtherConnectionSettings" => {
                Some(Self::AcceptsOtherConnectionSettings)
            }
            "AgentCapabilities_AcceptsRestartCommand" => {
                Some(Self::AcceptsRestartCommand)
            }
            "AgentCapabilities_ReportsHealth" => Some(Self::ReportsHealth),
            "AgentCapabilities_ReportsRemoteConfig" => Some(Self::ReportsRemoteConfig),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RemoteConfigStatuses {
    /// The value of status field is not set.
    Unset = 0,
    /// Remote config was successfully applied by the Agent.
    Applied = 1,
    /// Agent is currently applying the remote config that it received earlier.
    Applying = 2,
    /// Agent tried to apply the config received earlier, but it failed.
    /// See error_message for more details.
    Failed = 3,
}
impl RemoteConfigStatuses {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            RemoteConfigStatuses::Unset => "RemoteConfigStatuses_UNSET",
            RemoteConfigStatuses::Applied => "RemoteConfigStatuses_APPLIED",
            RemoteConfigStatuses::Applying => "RemoteConfigStatuses_APPLYING",
            RemoteConfigStatuses::Failed => "RemoteConfigStatuses_FAILED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "RemoteConfigStatuses_UNSET" => Some(Self::Unset),
            "RemoteConfigStatuses_APPLIED" => Some(Self::Applied),
            "RemoteConfigStatuses_APPLYING" => Some(Self::Applying),
            "RemoteConfigStatuses_FAILED" => Some(Self::Failed),
            _ => None,
        }
    }
}
/// The status of this package.
/// Status: \[Beta\]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PackageStatusEnum {
    /// Package is successfully installed by the Agent.
    /// The error_message field MUST NOT be set.
    Installed = 0,
    /// Installation of this package has not yet started.
    InstallPending = 1,
    /// Agent is currently downloading and installing the package.
    /// server_offered_hash field MUST be set to indicate the version that the
    /// Agent is installing. The error_message field MUST NOT be set.
    Installing = 2,
    /// Agent tried to install the package but installation failed.
    /// server_offered_hash field MUST be set to indicate the version that the Agent
    /// tried to install. The error_message may also contain more details about
    /// the failure.
    InstallFailed = 3,
}
impl PackageStatusEnum {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            PackageStatusEnum::Installed => "PackageStatusEnum_Installed",
            PackageStatusEnum::InstallPending => "PackageStatusEnum_InstallPending",
            PackageStatusEnum::Installing => "PackageStatusEnum_Installing",
            PackageStatusEnum::InstallFailed => "PackageStatusEnum_InstallFailed",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "PackageStatusEnum_Installed" => Some(Self::Installed),
            "PackageStatusEnum_InstallPending" => Some(Self::InstallPending),
            "PackageStatusEnum_Installing" => Some(Self::Installing),
            "PackageStatusEnum_InstallFailed" => Some(Self::InstallFailed),
            _ => None,
        }
    }
}
