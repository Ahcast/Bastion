#![forbid(unsafe_code)]

mod client_approval;
mod client_requests;
mod consent;
mod credential;
mod discovery;
mod dns_policy;
mod exchange;
mod legacy;
mod metadata;
mod pkce;
mod policy;
mod protocol;
mod quota;
mod registry;
mod storage;
mod token_exchange;
mod transport;
mod validity;

pub use client_approval::{ApprovedClientStore, ClientApprovalError};
pub use client_requests::{
    ClientRequestError, ClientRequestPage, ClientRequestStore, ClientRequestSummary, ReviewDecision,
};
pub use consent::{
    ConsentError, ConsentIdentity, ConsentPolicy, ConsentResult, DeniedConsent, MysqlConsent,
    PendingConsent,
};
pub use credential::{Credential, CredentialError, CredentialKind};
pub use discovery::{DiscoveryError, dispatch_discovery};
pub use dns_policy::{DnsPolicyError, PublicDnsCandidates, validate_public_dns_candidates};
pub use exchange::{
    CodeRecord, ExchangeError, RefreshRecord, validate_code_exchange, validate_refresh_exchange,
};
pub use legacy::{LegacyDispatch, LegacyMessage, dispatch_legacy, validate_legacy_message};
pub use metadata::{
    ClientMetadata, MetadataError, validate_client_identifier, validate_client_metadata,
    validate_redirect_uri,
};
pub use pkce::{PkceError, S256Challenge};
pub use policy::{
    AuthorizationRequest, ClientApproval, PolicyError, validate_authorization_request,
};
pub use protocol::{
    AuthorizationParameters, ProtocolError, TokenParameters, parse_authorization_parameters,
    parse_token_parameters,
};
pub use quota::{ActorQuota, MysqlQuota, QuotaError, ToolQuota};
pub use registry::{RegistryError, ToolDescriptor, ToolRegistry, ToolRisk};
pub use storage::{MysqlOauthStore, StoreError, revoke_user_grants_in_transaction};
pub use token_exchange::{
    AuthenticatedAccess, ExchangePolicy, IssuedTokens, MysqlTokenExchange, TokenExchangeError,
};
pub use transport::{
    ModernHeaders, ModernRequest, TransportError, extract_request_id, validate_modern_request,
};
pub use validity::{GrantState, TokenWindow, ValidityError, validate_token_window};

// MCP transport stays disabled until OAuth consent, delegated identity,
// per-call authorization, human confirmation, rate limits, and audit hooks
// are implemented together.
