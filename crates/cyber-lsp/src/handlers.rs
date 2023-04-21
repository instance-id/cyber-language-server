use std::path::Path;
use std::time::Instant;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tracing::debug;
use tracing::info;

use crate::Backend;
use crate::State;

use crate::completions;
use crate::datatypes::TextDocumentItem;
use crate::diagnostics::check_compile_error;
use crate::diagnostics::check_tree_error;
use crate::documents::FullTextDocument;
use crate::utils::treehelper;
use crate::utils::treehelper::TreeWrapper;
use crate::utils::treehelper::generate_lsp_range;
use crate::utils::treehelper::get_tree_sitter_edit_from_change;

// --| Backend Implementation ---------
// --|---------------------------------
impl Backend{
  // --| Get Urls ----------------
  pub async fn get_urls(&self) -> Vec<Url> {
    let docs = self.docs.lock().await;
    docs.iter().map(|(url, _)| url.clone()).collect::<Vec<Url>>()
  }

  // --| Initialize -----------------------------
  // --|-----------------------------------------
  // --| Initialize handler -----------
  pub async fn on_initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
    let capabilities = params.capabilities;
    let mut state = State::new();

    // Last I heard, only vscode supports dynamic_registration
    // nvim does not support dynamic or static registration.
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

      capabilities: ServerCapabilities {
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

  // --| Diagnostics ----------------------------
  // --|-----------------------------------------
  // --| Publish Diagnostics ----------
  pub async fn publish_diagnostics(&self, uri: Url, context: String) {
    // if context.is_empty() { return; }

    // let current_tree = self.parse_tree.lock().await;
    // let tree = current_tree.get(&uri).unwrap();

    // let mut parser = self.parser.lock().await; 
    // let _new_tree = parser.parse(&context, Some(tree)).unwrap();

    let uri_path = Path::new(uri.path());
    let diag_results = check_compile_error(&uri_path, &context);
    // let _tree_results = check_tree_error(&uri_path, &context, tree.root_node());

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

  // --| Updated diagnostics ----------
  pub async fn update_diagnostics(&self) {
    let urls = self.get_urls().await;

    info!("Update Diagnostics");
    let docs = &self.docs.lock().await;   

    for url in urls {
      let doc = docs.get(&url).unwrap();
      let context = doc.get_text();
      self.publish_diagnostics(url.clone(), context.to_string()).await;
    }
  }

  // --| Change Events -------------------------- 
  // --|-----------------------------------------
  // --| did_open handler -------------
  pub async fn on_open(&self, params: DidOpenTextDocumentParams) {
    info!("File Opened: {:?}", params.text_document.uri);
    let docs = &mut self.docs.lock().await; 

    let mut parser = self.parser.lock().await;
    let parse_tree = &mut self.parse_tree.lock().await;

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

  // --| onChange event handler -------
  pub async fn on_change(&self, params: DidChangeTextDocumentParams) {
    if params.content_changes.is_empty() { return; }

    if let Some(document) = self.docs.lock().await.get_mut(&params.text_document.uri) {
      let mut parser = self.parser.lock().await;
      let mut parse_tree = self.parse_tree.lock().await;
      let changes: Vec<TextDocumentContentChangeEvent> = params.content_changes.into_iter()
        .map(|change| {
          let range = change.range.map(|range| {
            generate_lsp_range(
              range.start.line as u32, range.start.character as u32,
              range.end.line as u32, range.end.character as u32,
              )
          });

          TextDocumentContentChangeEvent {
            range,
            range_length: change.range_length.and_then(|v| Some(v as u32)),
            text: change.text,
          }
        }).collect();

      let version = params.text_document.version;
      let tree = parse_tree.get_mut(&params.text_document.uri).unwrap();

      let start = Instant::now();
      for change in changes {
        let edits = &get_tree_sitter_edit_from_change(&change, document, version as i64);
        if let Some(edits) = edits { tree.edit(edits); }
      }

      debug!("Incremental updating: {:?}", start.elapsed());
      let new_tree = parser.parse(document.rope.to_string(), Some(tree)).unwrap();
      parse_tree.insert(params.text_document.uri, new_tree);
    }

    // if doc.line_count() < 1000 {
    //   self.publish_diagnostics(input.uri, doc.get_text().to_string()).await;
    // }
  }

  // --| didSave handler -------------
  pub async fn on_save(&self, params: DidSaveTextDocumentParams) {
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

  // --| didClose handler ------------
  pub async fn on_close(&self, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri;
    let docs = &mut self.docs.lock().await;
    let parse_tree = &mut self.parse_tree.lock().await;

    info!("Removing Document: {:?}", uri);
    docs.remove(&uri);
    parse_tree.remove(&uri);

    info!("File Closed: {:?}", uri);
    self.client.log_message(MessageType::INFO, "file closed!").await;
  }

  // --| Action Requests ------------------------
  // --|-----------------------------------------

  // --| Completion Handler -----------
  pub async fn on_completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
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

  // --| Hover Handler ----------------
  pub async fn on_hover(&self,   params: HoverParams) -> Result<Option<Hover>> {
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
        let mut parser = self.parser.lock().await; 
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
              hover_str = format!( "
```cyber
{}
```
---
{}

```cyber
{}
``` ", result.keyword,  result.description, result.example);
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
}
 
