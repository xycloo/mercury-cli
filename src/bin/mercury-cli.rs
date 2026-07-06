// NB: this is the Retroshades CLI. `deploy` and `gen-key` use current endpoints.
// The `query`, `contracts`, and `list` subcommands still point at older endpoint paths
// that were since renamed (see the NB notes on each), so they need updating.
// main.rs
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, time::Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RetroshadeProgramIdentity {
    project_name: String,
    contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodeUploadClient {
    code: Vec<u8>,
    project_name: String,
    contracts: Vec<String>,
}

#[derive(Parser, Debug)]
#[command(
    name = "retroshade",
    version,
    about = "Minimal CLI for Retroshades endpoints"
)]
struct Cli {
    #[arg(long = "base", global = true, default_value = "http://127.0.0.1:8084")]
    base_url: String,
    #[arg(long = "jwt", global = true, default_value = "")]
    jwt: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Deploy {
        #[arg(long)]
        project: String,
        #[arg(long = "code-path")]
        code_path: String,
        #[arg(long, default_value = "")]
        contracts: String,
    },
    GenKey,
    Query {
        #[arg(long)]
        sql: String,
    },
    Contracts {
        #[arg(long)]
        project: String,
        #[arg(long, default_value = "")]
        contracts: String,
    },
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?;

    let result = match cli.cmd {
        Cmd::Deploy {
            project,
            code_path,
            contracts,
        } => {
            let code = fs::read(&code_path)?;
            let body = CodeUploadClient {
                code,
                project_name: project,
                contracts: csv_to_vec(&contracts),
            };
            post_json(
                &client,
                &format!("{}/retroshade/deploy", trim(&cli.base_url)),
                auth(&cli.jwt),
                &body,
            )
            .await
        }
        Cmd::GenKey => {
            post_empty(
                &client,
                &format!("{}/v2/key", trim(&cli.base_url)),
                auth(&cli.jwt),
            )
            .await
        }
        Cmd::Query { sql } => {
            let body = json!({ "query": sql });
            post_json_value(
                &client,
                // NB: stale -- endpoint renamed to /retroshade/query; this path no longer exists.
                &format!("{}/retroshadesv1", trim(&cli.base_url)),
                auth(&cli.jwt),
                &body,
            )
            .await
        }
        Cmd::Contracts { project, contracts } => {
            let body = RetroshadeProgramIdentity {
                project_name: project,
                contracts: csv_to_vec(&contracts),
            };
            post_json(
                &client,
                // NB: stale -- endpoint renamed to /retroshade/append-contracts (request body shape also changed).
                &format!("{}/retroshades/contracts", trim(&cli.base_url)),
                auth(&cli.jwt),
                &body,
            )
            .await
        }
        Cmd::List => {
            get_json(
                &client,
                // NB: stale -- endpoint renamed to /retroshade/list (and /retroshade/tables).
                &format!("{}/retroshades", trim(&cli.base_url)),
                auth(&cli.jwt),
            )
            .await
        }
    };

    match result {
        Ok(s) => print_success(&s),
        Err(e) => print_error(&e),
    }
    Ok(())
}

fn trim(s: &str) -> String {
    s.trim_end_matches('/').to_string()
}
fn csv_to_vec(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .map(|x| x.to_string())
        .collect()
}
fn auth(jwt: &str) -> Option<(String, String)> {
    let t = jwt.trim();
    if t.is_empty() {
        None
    } else {
        Some(("Authorization".into(), format!("Bearer {}", t)))
    }
}
async fn post_json<T: serde::Serialize>(
    client: &reqwest::Client,
    url: &str,
    auth: Option<(String, String)>,
    body: &T,
) -> Result<String, String> {
    let mut req = client.post(url).json(body);
    if let Some((k, v)) = auth {
        req = req.header(k, v);
    }
    send(req).await
}
async fn post_json_value(
    client: &reqwest::Client,
    url: &str,
    auth: Option<(String, String)>,
    body: &serde_json::Value,
) -> Result<String, String> {
    let mut req = client.post(url).json(body);
    if let Some((k, v)) = auth {
        req = req.header(k, v);
    }
    send(req).await
}
async fn post_empty(
    client: &reqwest::Client,
    url: &str,
    auth: Option<(String, String)>,
) -> Result<String, String> {
    let mut req = client.post(url);
    if let Some((k, v)) = auth {
        req = req.header(k, v);
    }
    send(req).await
}
async fn get_json(
    client: &reqwest::Client,
    url: &str,
    auth: Option<(String, String)>,
) -> Result<String, String> {
    let mut req = client.get(url);
    if let Some((k, v)) = auth {
        req = req.header(k, v);
    }
    send(req).await
}
async fn send(req: reqwest::RequestBuilder) -> Result<String, String> {
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if status.is_success() {
        Ok(text)
    } else {
        Err(format!("HTTP {}: {}", status.as_u16(), text))
    }
}
fn print_success(s: &str) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        println!(
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or_else(|_| s.to_string())
        );
    } else {
        println!("{}", s);
    }
}
fn print_error(e: &str) {
    eprintln!("{}", e);
}
