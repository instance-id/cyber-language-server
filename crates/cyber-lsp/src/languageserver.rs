use std::path::Path;

use cyber_tree_sitter::Node;
use cyber_tree_sitter::Tree;
use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;
use tracing::info;

use crate::State;
use crate::documents::FullTextDocument;
use crate::{completions, Backend};
use crate::diagnostics::error_check;
use crate::utils::treehelper;

impl Backend {

  pub async fn get_urls(&self) -> Vec<Url> {
   let docs = self.docs.lock().await;
   docs.iter().map(|(url, _)| url.clone()).collect::<Vec<Url>>()
  }

  // --| Check and publish the initial diagnostics.
  async fn publish_diagnostics(&self, uri: Url, context: String) {
    if context.is_empty() { return; }

    let diag_results = error_check(Path::new(uri.path()), &context);
    
    if let Some(diag) = diag_results {
      let mut diagnostic_items = vec![];

      for (start, end, message, severity) in diag.inner {
        let pointx = lsp_types::Position::new(start.row as u32, start.column as u32);
        let pointy = lsp_types::Position::new(end.row as u32, end.column as u32);
        let range = Range { start: pointx, end: pointy };

        let diagnose = Diagnostic { 
          range, severity, code: None, code_description: None,
          source: None, message, related_information: None, tags: None, data: None,
        };

        diagnostic_items.push(diagnose);
      }
      self.client.publish_diagnostics(uri, diagnostic_items, Some(1)).await;
    } else {
      self.client.publish_diagnostics(uri, vec![], None).await;
    }
  }

  // --| Publishes the updated diagnostics.
  async fn update_diagnostics(&self) {
    let urls = self.get_urls().await;

    info!("Update Diagnostics");
    let docs = &self.docs.lock().await;   

    for url in urls {
      let doc = docs.get(&url).unwrap();
      let context = doc.get_text();
      self.publish_diagnostics(url.clone(), context.to_string()).await;
    }
  }
}

struct TextDocumentItem {
  uri: Url,
  text: String,
  version: i32,
  changes: Vec<TextDocumentContentChangeEvent>
}

impl Backend {
  async fn on_change(&self, input: TextDocumentItem) {
    let _parser = cyber_tree_sitter::try_init_parser().expect("Parser failed to load");

    let docs = &mut self.docs.lock().await;
    info!("Retrieved document data");

    docs.get_mut(&input.uri).unwrap().update(input.changes, input.version.into());
    info!("Updated document data");

    self.client.log_message(MessageType::INFO, "file changed!").await;
    self.client.log_message(MessageType::INFO, &format!("{:?}", input.text)).await;

    // if docs.line_count() < 1000 {
    //   self.publish_diagnostics(input.uri, docs.get_text().to_string()).await;
    // }
    // let _doc = self.documents.get_document(&input.uri);
    // let rope = ropey::Rope::from_str(&input.text);
    // self.doc_map.insert(input.uri, rope.clone());
  }
}

// --| Language Server Protocol (LSP) implementation
#[tower_lsp::async_trait]
impl LanguageServer for Backend {

  async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
    let capabilities = params.capabilities;
    let mut state = State::new();

    // vscode only supports dynamic_registration
    // neovim supports neither dynamic or static registration of this yet.
    state.client_monitor = capabilities.workspace.map_or(false, |wrk| {
      wrk.did_change_watched_files.map_or(false, |dynamic| {
        dynamic.dynamic_registration.unwrap_or(false)
      })
    });

    if let Some(folders) = params.workspace_folders {
      folders.into_iter().for_each(|folder| {
        let uri_str = folder.uri.to_string(); 
        self.workspace_map.insert(folder.uri, folder.name.to_string());
        info!("Workspace: {} {}", uri_str, &folder.name);
      });
    }

    let pattern = GlobPattern::String("**/*.{cy,cyber}".to_string());

    // --| Register current workspace file watcher
    let registration = Registration {
      id: "cyber-source-files".to_string(),
      method: "workspace/didChangeWatchedFiles".to_string(),
      register_options: Some(
        serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
          watchers: vec![FileSystemWatcher {
            glob_pattern: pattern, 
            kind: None,
          }]
        }).unwrap_or_default(),
        ) 
    };

    let registrations = vec![registration];
    let _ = self.client.register_capability(registrations).await;

    Ok(InitializeResult {
      server_info: None,

      // --| Specify the current capabilities of the server
      capabilities: ServerCapabilities {
        // text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::INCREMENTAL)),
        text_document_sync: Some( TextDocumentSyncCapability::Options(
          TextDocumentSyncOptions {
            open_close: Some(true),
            will_save: Some(false),
            will_save_wait_until: Some(false),
            change: Some(TextDocumentSyncKind::FULL),
            save: Some(lsp_types::TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
              include_text: Some(true),
            })),
          })),

        completion_provider: Some(CompletionOptions {
          resolve_provider: Some(false),
          trigger_characters: Some(vec![".".to_string()]),
          work_done_progress_options: Default::default(),
          all_commit_characters: None,
          ..Default::default()
        }),

        execute_command_provider: Some(ExecuteCommandOptions {
          commands: vec!["dummy.do_something".to_string()],
          work_done_progress_options: Default::default(),
        }),

        hover_provider: Some(HoverProviderCapability::Simple(true)),

        workspace: Some(WorkspaceServerCapabilities {
          workspace_folders: Some(WorkspaceFoldersServerCapabilities {
            supported: Some(true),
            change_notifications: Some(OneOf::Left(true)),
          }),
          file_operations: None,
        }),

        ..ServerCapabilities::default()
      },
      ..Default::default()
    })
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
  // --|-------------------------------
  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    info!("File Opened: {:?}", params.text_document.uri);
    let docs = &mut self.docs.lock().await; 
    
    let parse_tree = &mut self.parse_tree.lock().await;
    let mut parser = cyber_tree_sitter::try_init_parser().expect("Parser failed to load");

    let uri = &params.text_document.uri;
    let context = params.text_document.text;
    let id = params.text_document.language_id;
    let version: i64 = params.text_document.version.into();

    info!("Creating Document Objects");
    let document = FullTextDocument::new(uri.clone(), id, version, context,);

    info!("Inserting Document Objects");
    docs.insert(document.uri.clone(), document.clone());

    info!("Retrieving Document Objects");
    let content =  document.get_text();

    if Some(content) == None {
      info!("Failed to get document content: {:?}", uri);
      self.client.log_message(MessageType::ERROR, format!("Failed to get document content: {:?}", uri)).await;
      return;
    }
    else{
      let tree = &parser.parse(content, None);
      if let Some(tree) = tree {
        info!("Inserting Parse Tree");
        parse_tree.insert(uri.clone(), tree.clone());
        info!("{}", TreeWrapper(tree.clone()));
      }

      info!("Begin Publishing Diagnostics: {:?}", uri.clone());
      self.publish_diagnostics(uri.clone(), content.to_string()).await;

      info!("Diagnostic Published: {:?}", uri.clone());
      self.client.log_message(MessageType::INFO, format!("file opened: {:?}", uri)).await;
    }
  }


  // --| File Change ------------------
  // --|-------------------------------
  async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
    info!("Did Change");

    self.on_change(TextDocumentItem {
      uri: params.text_document.uri,
      text: std::mem::take(&mut params.content_changes[0].text),
      version: params.text_document.version,
      changes: params.content_changes.clone(),
    }).await;

    // let rope = ropey::Rope::from_str(&params.text_document);
    // self.doc_map.insert(params.uri.to_string(), rope.clone());
    // info!("Created rope");

    // let input_data = params.clone();
    // let increment = &params.content_changes.clone();

    // let new_text = std::mem::take(&mut params.content_changes[0].text);

    // let docs = &mut self.docs.get_mut(&params.text_document.uri).unwrap();
    // info!("Retrieved document data");
    //
    // docs.update(increment.to_vec(), params.text_document.version.into());
    // info!("Updated document data");

    // let docs = &self.documents;
    // docs.listen(&method, &param);


    // let uri = input_data.text_document.uri.clone();
    // let context = input_data.content_changes[0].text.clone();
    //
    // info!("File Change: {:?} ctx: {:?} ", input_data, context);
    //
    // let mut storemap = self.buffers.lock().await;
    // storemap.insert(uri.clone(), context.clone());
    //
    // if context.lines().count() < 1000 {
    //   self.publish_diagnostics(uri, context).await;
    // }
  }

  // --| File Save --------------------
  // --|-------------------------------
  async fn did_save(&self, params: DidSaveTextDocumentParams) {
    let content = params.text;
    let uri = params.text_document.uri;

    if let Some(text) = content {
      info!("Begin Publishing Diagnostics: {:?}", uri.clone());
      self.publish_diagnostics(uri.clone(), text.to_string()).await;
    }
    else{
      info!("Failed to get document content: {:?}", uri);
      self.client.log_message(MessageType::ERROR, format!("Failed to get document content: {:?}", uri)).await;
      return;
    }

    info!("File Saved: {:?}", uri);
    self.client.log_message(MessageType::INFO, "file saved!").await;
  }

  // --| File Close -------------------
  // --|-------------------------------
  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    let docs = &mut self.docs.lock().await;

    docs.remove(&params.text_document.uri).unwrap();
    info!("File Closed: {:?}", params.text_document.uri);
    
    self.client.log_message(MessageType::INFO, "file closed!").await;
  }

  // --| Completion Request -----------
  // --|-------------------------------
  async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
    self.client.log_message(MessageType::INFO, "Completion Requested").await;
    let location = params.text_document_position.position;

    info!("Completion Requested: {:?}", params);

    if params.context.is_some() {
      let uri = params.text_document_position.text_document.uri;

      let tmp = &mut self.docs.lock().await;
      let doc_tmp = tmp.get_mut(&uri).unwrap();

      let doc_data = doc_tmp.get_text();
      if doc_data.len() == 0 { info!("Completion: No document found"); return Ok(None); }

      info!("Context is Some() requesting getcomplete({:?}, {:?}, {:?})", &self.client, location, uri.path());

      match Some(doc_data) {
        Some(context) => Ok(completions::get_completion(context, location, &self.client, uri.path()).await),
        None => { info!("storemap.get was no? Context is None"); Ok(None) }
      }
    } else {
      info!("storemap.get was no? Context is None");
      Ok(None)
    }
  }

  // --| Hover Request ----------------
  // --|-------------------------------
  async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
    let position = params.text_document_position_params.position;
    let uri = params.text_document_position_params.text_document.uri;

    let tmp = &mut self.docs.lock().await;
    let doc_tmp = tmp.get_mut(&uri).unwrap();

    let doc_data = doc_tmp.get_text();

    self.client.log_message(MessageType::INFO, "Hovered!").await;

    info!("Hover Requested");

    if doc_data.len() == 0 {
      info!("Hover: No document found");
      return Ok(None);
    }
    
    else if doc_data.lines().count() > 1000 {
      info!("Hover: Document too large");
      return Ok(None);
    }

    match Some(doc_data) {
      Some(context) => {
        let mut parser = cyber_tree_sitter::try_init_parser().expect("Parser failed to load");
        info!("Hover: Parser Loaded");

        let ts_tree = parser.parse(context.clone(), None);
        let tree = ts_tree.unwrap();

        info!("Hover: Looking up token at position: {:?} ctx: {:?} tree: {:?}", position, context, tree.root_node());

        let lsp_action = "hover".to_string();

        let output = treehelper::get_from_position(position, tree.root_node(), context, lsp_action);
        if output.is_none() {
          info!("Hover: No token found");
        }

        match output {
          Some(result) => {
            let hover_str: String;
            if self.lsp_client == "vscode" {
              hover_str  = format!("
### {} 
<p align='right'>{}</p>

---
#### {}  

```cyber
{}
```  ", result.keyword, result.keyword_detail_type, result.description, result.example);
              } else {
                hover_str = format!("
```cyber
{}
```
---
{}

```cyber
{}
```  ", result.keyword,  result.description, result.example);
                }

            Ok(Some(Hover {
              contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover_str,
              }),
              range: Some(Range { start: position, end: position }),
            }))
          },
          None => Ok(None),
        }
      }
      None => Ok(None),
    }
  }

  // --| Workspace Change -------------
  // --|-------------------------------
  async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
    self.client.log_message(MessageType::INFO, "workspace folders changed!").await;
  }

  // --| Configuration Change ---------
  // --|-------------------------------
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
  async fn shutdown(&self) -> Result<()> {
    Ok(())
  }
}


pub struct TreeWrapper(pub Tree);
impl std::fmt::Display for TreeWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty_display(f, self.0.root_node())?;
        Ok(())
    }
}

pub fn pretty_display(f: &mut std::fmt::Formatter<'_>, root: Node) -> std::fmt::Result {
    let mut stack = Vec::new();
    if !root.is_named() {
        return Ok(());
    }
    stack.push((root, 0));
    while let Some((node, level)) = stack.pop() {
        let kind = node.kind();
        let start = node.start_position();
        let end = node.end_position();
        info!("{}{} [{}, {}] - [{}, {}] ", " ".repeat(level * 2), kind, start.row, start.column, end.row, end.column);
        writeln!(
            f,
            "{}{} [{}, {}] - [{}, {}] ",
            " ".repeat(level * 2),
            kind,
            start.row,
            start.column,
            end.row,
            end.column
        )?;
        for i in (0..node.named_child_count()).rev() {
            let child = node.named_child(i).unwrap();
            stack.push((child, level + 1));
        }
    }
    Ok(())
}
