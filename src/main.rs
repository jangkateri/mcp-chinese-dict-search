//! Executable entry point.
//!
//! Kept trivial on purpose: the library owns configuration, logging, protocol
//! dispatch, the dictionary, and the stdio transport.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let code = mcp_chinese_dict_search::cli::run().await;
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
