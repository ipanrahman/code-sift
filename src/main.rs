//! CodeSift CLI.

use clap::{Parser, ValueEnum};
use codesift::{mcp::JsonRpcRequest, CodeSift, McpServer, TokenBudget};
use std::io::{self, BufRead, Write};

#[derive(Parser, Debug)]
#[command(name = "codesift")]
#[command(about = "Token-efficient code intelligence engine for AI agents")]
struct Args {
    /// Repository path to index
    #[arg(short, long, default_value = ".")]
    repo: std::path::PathBuf,

    /// Search query
    #[arg(last = true)]
    query: Option<String>,

    /// Search mode
    #[arg(short, long, default_value = "symbol")]
    mode: SearchMode,

    /// Maximum tokens to return
    #[arg(short, long, default_value = "2000")]
    max_tokens: usize,

    /// Maximum files to include
    #[arg(short, long, default_value = "10")]
    max_files: usize,

    /// Maximum symbols to return
    #[arg(long, default_value = "20")]
    max_symbols: usize,

    /// Output format
    #[arg(short, long, default_value = "compact")]
    format: OutputFormat,

    /// MCP server mode (JSON-RPC 2.0)
    #[arg(long)]
    mcp: bool,

    /// Watch repository for filesystem changes (incremental re-index)
    #[arg(long)]
    watch: bool,

    /// Show statistics
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone, ValueEnum)]
enum SearchMode {
    Symbol,
    References,
    Callers,
    Callees,
    Lexical,
    Context,
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Json,
    Jsonl,
    Compact,
    Full,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize CodeSift
    let codesift = CodeSift::open(&args.repo)?;

    if args.mcp {
        run_mcp_server(codesift)
    } else if args.watch {
        run_watch(codesift)
    } else {
        run_cli(codesift, args)
    }
}

fn run_cli(codesift: CodeSift, args: Args) -> anyhow::Result<()> {
    let budget = TokenBudget::new(args.max_tokens)
        .with_files(args.max_files)
        .with_symbols(args.max_symbols);

    if args.verbose {
        eprintln!("Indexed {} files, {} symbols", codesift.file_count(), codesift.symbol_count());
    }

    let query = args.query.unwrap_or_default();

    match args.format {
        OutputFormat::Json | OutputFormat::Jsonl => {
            let plan = codesift.plan_context(&query, Some(budget))?;

            for fragment in &plan.fragments {
                let file = codesift.get_file(fragment.file_id).unwrap();
                let result = serde_json::json!({
                    "file": file.path.to_string_lossy(),
                    "lines": format!("{}-{}", fragment.range.start_line, fragment.range.end_line),
                    "symbol": fragment.symbol_name,
                    "content": fragment.content,
                });

                println!("{}", result);
            }
        }
        OutputFormat::Compact => {
            let plan = codesift.plan_context(&query, Some(budget))?;

            if plan.is_empty() {
                println!("No results found for: {}", query);
                return Ok(());
            }

            // Group by file
            let mut files: std::collections::HashMap<String, Vec<&codesift::ContextFragment>> =
                std::collections::HashMap::new();

            for frag in &plan.fragments {
                if let Some(file) = codesift.get_file(frag.file_id) {
                    let path = file.path.to_string_lossy().to_string();
                    files.entry(path).or_default().push(frag);
                }
            }

            // Print compact format
            for (path, frags) in &files {
                let symbols: Vec<&str> = frags
                    .iter()
                    .filter_map(|f| f.symbol_name.as_deref())
                    .collect();
                let lines = frags
                    .iter()
                    .map(|f| format!("{}-{}", f.range.start_line, f.range.end_line))
                    .collect::<Vec<_>>()
                    .join(",");

                println!("{}:{} {:?}", path, lines, symbols);
            }

            println!();
            println!("Tokens: {}/{}", plan.total_tokens, budget.max_tokens);
            println!("Files: {}", plan.total_files);
            println!("Symbols: {}", plan.len());
        }
        OutputFormat::Full => {
            let plan = codesift.plan_context(&query, Some(budget))?;

            for frag in &plan.fragments {
                if let Some(file) = codesift.get_file(frag.file_id) {
                    println!("=== {}:{} ===", file.path.to_string_lossy(), frag.range.start_line);
                    if let Some(name) = &frag.symbol_name {
                        println!("Symbol: {}", name);
                    }
                    println!("{}", frag.content);
                    println!();
                }
            }
        }
    }

    if args.verbose {
        eprintln!("Indexed {} files, {} symbols", codesift.file_count(), codesift.symbol_count());
    }

    Ok(())
}

fn run_watch(mut codesift: CodeSift) -> anyhow::Result<()> {
    codesift.watch()?;
    Ok(())
}

fn run_mcp_server(codesift: CodeSift) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut server = McpServer::new(codesift);

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // Parse JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": serde_json::Value::Null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {}", e)
                    }
                });
                writeln!(stdout, "{}", response)?;
                stdout.flush()?;
                continue;
            }
        };

        let response = server.handle(request);
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }

    Ok(())
}
