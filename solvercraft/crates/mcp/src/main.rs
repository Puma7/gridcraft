//! Workbook-independent, on-demand Solvercraft MCP entry point.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo)]
use solvercraft_mcp::{Config, Server};
use std::io::Read;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] || args == ["-h"] {
        println!(
            "solvercraft-mcp [--config PATH]\nStandalone MCP over stdin/stdout. No network listener or spreadsheet.\nStart this process from an MCP client; close stdin to cancel jobs and exit."
        );
        return Ok(());
    }
    if args == ["--version"] {
        println!("solvercraft-mcp {}", solvercraft::VERSION);
        return Ok(());
    }
    let config = if args.is_empty() {
        Config::default()
    } else if args.len() == 2 && args.first().is_some_and(|a| a == "--config") {
        let path = args.get(1).ok_or("Missing config path")?;
        let mut bytes = Vec::new();
        std::fs::File::open(path)?.take(65537).read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err("MCP config exceeds 64 KiB".into());
        }
        serde_json::from_slice(&bytes)?
    } else {
        return Err("Usage: solvercraft-mcp [--config PATH]".into());
    };
    let mut server = Server::new(config)?;
    server.serve(std::io::stdin().lock(), std::io::stdout().lock())?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{}", serde_json::json!({"error":{"code":"mcp_process","message":error.to_string()}}));
        std::process::exit(1);
    }
}
