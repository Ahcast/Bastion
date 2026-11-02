use std::time::Duration;

use bastion_mcp_http::DraftMetadataFetcher;

#[tokio::main]
async fn main() {
    let fetcher = DraftMetadataFetcher::new(Duration::from_secs(5)).expect("valid timeout");
    match fetcher.fetch("https://cursor.com/oauth/mcp-client.json").await {
        Ok(metadata) => {
            println!("METADATA_FETCH_OK");
            println!("REDIRECTS={}", metadata.redirect_uris().len());
        }
        Err(error) => {
            eprintln!("METADATA_FETCH_ERROR={error:?}");
            std::process::exit(1);
        }
    }
}
