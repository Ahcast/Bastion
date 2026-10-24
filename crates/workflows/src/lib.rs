#![forbid(unsafe_code)]

//! Composition between domains through explicit transactional ports.
//! Each module retains ownership of its tables, authorization and business rules.
mod conversion;
pub use conversion::{QuoteOrderSeed, convert_quote_order, prepare_quote_order, seed_quote_order};
