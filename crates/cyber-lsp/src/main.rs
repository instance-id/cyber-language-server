use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

use lsp_types::Url;
use dashmap::DashMap;
use tokio::sync::Mutex;
use tokio::net::TcpListener;
use clap::{arg, Arg, Command};
use cyber_tree_sitter::{Tree, Parser};
use tower_lsp::{LspService, Server};

use tracing::info;
use tracing_subscriber::filter;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

use datatypes::LogData;
use crate::documents::FullTextDocument;

mod utils;
mod macros;
mod handlers;
mod documents;
mod datatypes;
mod completions;
mod diagnostics;
mod languageserver;
mod semantic_tokens;

struct Backend {
  pub(crate) log_data: LogData,
  pub(crate) lsp_client: String,
  pub(crate) parser: Mutex<Parser>,
  pub(crate) client: tower_lsp::Client,
  pub(crate) parse_tree:Mutex<HashMap<Url, Tree>>,
  pub(crate) docs: Arc<Mutex<HashMap<lsp_types::Url, FullTextDocument>>>,
  pub workspace_map: DashMap<Url, String>,
}

struct State {
  client_monitor: bool,
  _warned_needs_restart: bool,
  _shutdown: tokio::sync::broadcast::Sender<()>,
}

impl State {
  pub fn new() -> Self {
    let (_shutdown, _) = tokio::sync::broadcast::channel(1);
    Self { client_monitor: false, _shutdown, _warned_needs_restart: false}}
}

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref DOCUMENTS: RwLock<DashMap<Url, FullTextDocument>> = RwLock::new(DashMap::new());
}

#[tokio::main]
async fn main() {
  const VERSION: &str = env!("CARGO_PKG_VERSION");

  let matches = 
    Command::new("cyberls")
    .about("cyber language server")
    .version(VERSION)
    .subcommand_required(true)
    .arg_required_else_help(true)
    .author("instance.id")

    .arg( // --| Lsp connection client ----------
      arg!(client: -c --client <CLIENT> "The client to use")
      .default_value("vscode").default_missing_value("nvim")
      .value_parser(["vscode", "nvim"]))

    .arg( // --| Log level ----------------------
      arg!(level: -l --level <Name> "The log level to use")
      .default_value("info").default_missing_value("info")
      .value_parser(["error", "warn", "info", "debug"]))

    .arg( // --| Verbose ------------------------      
      arg!(verbose: -v --verbose "Enable verbose logging")
      .action(clap::ArgAction::SetTrue))
    
    .subcommand( // --| Sdtio Communication -----
      Command::new("stdio").long_flag("stdio").about("communicate via stdio"))
    
    .subcommand( // --| TCP Communication -------
      Command::new("tcp").long_flag("tcp").about("run with tcp").arg(
        Arg::new("port").long("port").short('P').help("listen to port")))
    .get_matches();

  // Remove log file if exists
  let log_file_dir = std::env::current_exe().unwrap().with_file_name("");
  let log_file = log_file_dir.join("cyberls.log");
  if log_file.exists() {
    std::fs::remove_file(log_file).expect("error");
  }

  let log_level = matches.get_one::<String>("level").expect("error");
  let log_file_dir = std::env::current_exe().unwrap().with_file_name("");
  let file_appender = RollingFileAppender::new(Rotation::NEVER, log_file_dir, "cyberls.log");
  let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

  let filter = match log_level.as_str() {
    "info" => filter::LevelFilter::INFO,
    "warn" => filter::LevelFilter::WARN,
    "error" => filter::LevelFilter::ERROR,
    "debug" => filter::LevelFilter::DEBUG,
    _ => filter::LevelFilter::INFO,
  };

  tracing_subscriber::fmt()
    .with_ansi(false)
    .with_max_level(filter)
    .with_line_number(true)
    .with_writer(non_blocking)
    .init();

  // --| Sdtio Communication -----
  match matches.subcommand() {
    Some(("stdio", _)) => {
      
      let verbose = matches.get_flag("verbose");
      let lsp_client = matches.get_one::<String>("client").expect("error");
      let log_data = LogData::new(filter, verbose);

      info!("Client Connected: {} Log Level: {} Verbose: {}", &lsp_client, &log_data.log_level, &log_data.verbose);

      let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
      let (service, socket) = LspService::new(|client| Backend {
        client, log_data,
        workspace_map: DashMap::new(),  
        lsp_client: lsp_client.clone(),
        parse_tree: Mutex::new(HashMap::new()),
        docs: Arc::new(Mutex::new(HashMap::new())),
        parser: Mutex::new(cyber_tree_sitter::init_parser()),
      });

      info!("Starting cyberls server");
      Server::new(stdin, stdout, socket).serve(service).await;
    }

    // --| TCP Communication -----
    Some(("tcp", arguments)) => {
      #[cfg(feature = "runtime-agnostic")]
      use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

      let verbose = matches.get_flag("verbose");
      let lsp_client = matches.get_one::<String>("client").expect("error");
      let log_data = LogData::new(filter, verbose);

      let stream = {
        // --| Use port if provided
        if arguments.contains_id("port") {
          let port = arguments.get_one::<String>("port").expect("error");
          let port: u16 = port.parse().unwrap();

          let listener = TcpListener::bind(
            SocketAddr::new( std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
          ).await.unwrap();

          let (stream, _) = listener.accept().await.unwrap();
          stream
        }

        // --| Use default port
        else {
          let listener = TcpListener::bind(
            SocketAddr::new( std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9257)
          ).await.unwrap();

          let (stream, _) = listener.accept().await.unwrap();
          stream
        }
      };

      let (read, write) = tokio::io::split(stream);

      #[cfg(feature = "runtime-agnostic")]
      let (read, write) = (read.compat(), write.compat_write());

      let (service, socket) = LspService::new(|client| Backend {
        client, log_data,
        workspace_map: DashMap::new(),  
        lsp_client: lsp_client.clone(), 
        parse_tree: Mutex::new(HashMap::new()),
        docs: Arc::new(Mutex::new(HashMap::new())),
        parser: Mutex::new(cyber_tree_sitter::init_parser()),
      });

      Server::new(read, write, socket).serve(service).await;
    }
    _ => unreachable!(),
  }
}

