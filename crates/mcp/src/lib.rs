#![forbid(unsafe_code)]

mod credential;
mod exchange;
mod pkce;
mod policy;
mod protocol;
mod storage;
mod validity;

pub use credential::{Credential, CredentialError, CredentialKind};
pub use exchange::{
    CodeRecord, ExchangeError, RefreshRecord, validate_code_exchange, validate_refresh_exchange,
};
pub use pkce::{PkceError, S256Challenge};
pub use policy::{
    AuthorizationRequest, ClientApproval, PolicyError, validate_authorization_request,
};
pub use protocol::{
    AuthorizationParameters, ProtocolError, TokenParameters, parse_authorization_parameters,
    parse_token_parameters,
};
pub use storage::{MysqlOauthStore, StoreError, revoke_user_grants_in_transaction};
pub use validity::{GrantState, TokenWindow, ValidityError, validate_token_window};

// MCP transport stays disabled until OAuth consent, delegated identity,
// per-call authorization, human confirmation, rate limits, and audit hooks
// are implemented together.
