use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

use clap::{arg, Arg, Command};
use dashmap::DashMap;
use documents::TextDocuments;
use ropey::Rope;
use lsp_types::Url;
use tower_lsp::{LspService, Server};
use tokio::sync::Mutex;
use tokio::net::TcpListener;

use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

use crate::documents::FullTextDocument;

mod complete;
mod completions;
mod datatypes;
mod diagnostics;
mod languageserver;
mod utils;
mod documents;
mod semantic_tokens;

#[derive(Debug)]
struct Backend {
  client: tower_lsp::Client,
  doc_map: DashMap<Url, Rope>,
  documents: TextDocuments,
  workspace_map: DashMap<Url, String>,
  pub docs: DashMap<lsp_types::Url, FullTextDocument>,
  buffers: Arc<Mutex<HashMap<Url, String>>>
}

#[derive(Debug)]
struct State {
    client_monitor: bool,
    shutdown: tokio::sync::broadcast::Sender<()>,
    warned_needs_restart: bool,
}

impl State {
  pub fn new() -> Self {
    let (shutdown, _) = tokio::sync::broadcast::channel(1);
    Self { client_monitor: false, shutdown, warned_needs_restart: false}}
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
    .arg(
      arg!(--level <Name>)
      .short('l').num_args(0..=1)
      .help("The log level to use").require_equals(true)
      .default_value("info").default_missing_value("info")
      .value_parser(["error", "warn", "info", "debug"]))
    // --| Sdtio Communication ---
    .subcommand(
      Command::new("stdio").long_flag("stdio").about("communicate via stdio"))

    // --| TCP Communication -----
    .subcommand(
      Command::new("tcp").long_flag("tcp").about("run with tcp").arg(
        Arg::new("port").long("port").short('P').help("listen to port")))
    .get_matches();


    let log_file_dir = std::env::current_exe().unwrap().with_file_name("");
    let file_appender = RollingFileAppender::new(Rotation::NEVER, log_file_dir, "cyberls.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

  let docs = &mut DOCUMENTS.write().unwrap();

  // --| Sdtio Communication -----
  match matches.subcommand() {
    Some(("stdio", _)) => {
      tracing_subscriber::fmt()
        .with_ansi(false)
        .with_line_number(true)
        .with_writer(non_blocking)
        .init();

      let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
      let (service, socket) = LspService::new(|client| Backend {
        client, 
        doc_map: DashMap::new(), 
        docs: DashMap::new(),
        documents: TextDocuments::new(),
        workspace_map: DashMap::new(),  
        buffers: Arc::new(Mutex::new(HashMap::new())),
      });

      info!("Starting server");
      Server::new(stdin, stdout, socket).serve(service).await;
    }

    // --| TCP Communication -----
    Some(("tcp", sync_matches)) => {
      #[cfg(feature = "runtime-agnostic")]
      use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

      tracing_subscriber::fmt().init();

      let stream = {
        // --| Use port if provided
        if sync_matches.contains_id("port") {
          let port = sync_matches.get_one::<String>("port").expect("error");
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
        client, 
        doc_map: DashMap::new(), 
        docs: docs.clone(),
        documents: TextDocuments::new(),
        workspace_map: DashMap::new(),  
        buffers: Arc::new(Mutex::new(HashMap::new())),
      });

      Server::new(read, write, socket).serve(service).await;
    }
    _ => unreachable!(),
  }
}