use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;
use tracing::info;

use crate::Backend;
use crate::datatypes::TextDocumentItem;

// --| Language Server Protocol (LSP) implementation
#[tower_lsp::async_trait]
impl LanguageServer for Backend {

  async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
    self.on_initialize(params).await
  }

  // --| Initialized ------------------
  // --|-------------------------------
  async fn initialized(&self, _: InitializedParams) {
    info!("Loading Cyber Language Definitions...");
    self.client.log_message(MessageType::INFO, "cyberls initialized").await;
  }

  // --| Execute Command -------
  async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
    self.client.log_message(MessageType::INFO, "command executed!").await;

    match self.client.apply_edit(WorkspaceEdit::default()).await {
      Ok(res) if res.applied => self.client.log_message(MessageType::INFO, "applied").await,
      Ok(_) => self.client.log_message(MessageType::INFO, "rejected").await,
      Err(err) => self.client.log_message(MessageType::ERROR, err).await,
    }
    Ok(None)
  }

  // --| File Open --------------------
  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    self.on_open(params).await;
  }

  // --| File Change ------------------
  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    self.on_change(params).await;
  }

  // --| File Save --------------------
  async fn did_save(&self, params: DidSaveTextDocumentParams) {
    self.on_save(params).await;
  }

  // --| File Close -------------------
  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    self.on_close(params).await; 
  }

  // --| Completion Request -----------
  async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
    self.on_completion(params).await 
  }

  // --| Hover Request ----------------
  async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
    self.on_hover(params).await
  }

  // --| Workspace Change -------------
  async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
    self.client.log_message(MessageType::INFO, "workspace folders changed!").await;
  }

  // --| Configuration Change ---------
  async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
    self.client.log_message(MessageType::INFO, "configuration changed!").await;
  }

  // --| Changed Watched Files --------
  // --|-------------------------------
  async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
    info!("Watched Files Changed");

    self.client.log_message(MessageType::INFO, "watched files have changed!").await;

    for change in params.changes {
      if let FileChangeType::DELETED = change.typ {
        // filewatcher::clear_error_packages();
      } else {
        let _path = change.uri.path();
        // filewatcher::refresh_error_packages(path);
      }
    }

    self.update_diagnostics().await;
    self.client.log_message(MessageType::INFO, "watched files have changed!").await;
  }

  // --| Shutdown ---------------------
  // --|-------------------------------
  async fn shutdown(&self) -> Result<()> { Ok(()) }
}

