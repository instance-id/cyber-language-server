use std::path::Path;
use std::time::Instant;

use cyber_tree_sitter::Tree;
use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::metadata::LevelFilter;

use crate::Backend;
use crate::State;

use crate::completions;
use crate::diagnostics::ErrorInfo;
use crate::documents::FullTextDocument;
use crate::diagnostics::{check_compile_error, check_tree_error};
use crate::utils::treehelper::get_parser_errors;
use crate::utils::treehelper::position_to_point;
use crate::utils::treehelper::{ TreeWrapper, get_range, get_tree_edits, get_from_position };

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
    let start = Instant::now();
    let mut state = State::new();
    
    let capabilities = params.capabilities;
    let options = params.initialization_options;
    debug!("Initialize: {:?}", options);

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

    debug!("Initialize: {:?}", start.elapsed().as_secs_f64());
    Ok(InitializeResult {
      server_info: None,

      capabilities: ServerCapabilities {
        text_document_sync: Some( TextDocumentSyncCapability::Options(
          TextDocumentSyncOptions {
            open_close: Some(true),
            will_save: Some(false),
            will_save_wait_until: Some(false),
            change: Some(TextDocumentSyncKind::INCREMENTAL),
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
  pub async fn publish_diagnostics(&self, uri: Url, errors: Option<ErrorInfo>) {
    if let Some(diag) = errors {
      let mut diagnostic_items = vec![];

      for err in diag.entries {
        let pointx = lsp_types::Position::new(err.start.row as u32, err.start.column as u32);
        let pointy = lsp_types::Position::new(err.end.row as u32, err.end.column as u32);
        let range = Range { start: pointx, end: pointy };

        let diagnose = Diagnostic { 
          range, severity: err.severity, code: None, code_description: None,
          source: None, message: err.message, related_information: None, tags: None, data: None,
        };

        diagnostic_items.push(diagnose);
      }

      debug!("Publish Diagnostics");
      self.client.publish_diagnostics(uri, diagnostic_items, Some(1)).await;
    } else {
      self.client.publish_diagnostics(uri, vec![], None).await;
    }
  }

  pub async fn obtain_basic_diagnostics(&self, uri: Url, context: String, tree: Tree) {
    let start = Instant::now();
    let errors = get_parser_errors(&context, Some(tree.clone()));

    let mut err_info: ErrorInfo = ErrorInfo::new();

    if errors.len() > 0 {
      for error in errors.iter() {
        err_info.add(
            position_to_point(error.start), 
            position_to_point(error.end), 
            "Syntax Error".to_string(), 
            Some(DiagnosticSeverity::ERROR)
           );
    }}

    debug!("Obtain Basic Diagnostics: {:?}", start.elapsed().as_secs_f64());
    self.publish_diagnostics(uri.clone(), Some(err_info)).await;
  }

  pub async fn obtain_full_diagnostics(&self, uri: Url, context: String) {
    let start = Instant::now();
    let mut errors = ErrorInfo::new();

    let uri_path = Path::new(uri.path());
    let mut diag_results = check_compile_error(&uri_path, &context);
    if diag_results.is_some() {
      errors.combine(diag_results.as_mut().unwrap());
    }

    let tree = self.parse_tree.lock().await.get(&uri).unwrap().clone();
    let mut tree_results = check_tree_error(&uri_path, &context, tree.root_node());
    if tree_results.is_some() {
      errors.combine(tree_results.as_mut().unwrap());
    }

    if errors.entries.len() == 0{ self.publish_diagnostics(uri.clone(), None).await; }
    else { self.publish_diagnostics(uri.clone(), Some(errors)).await; }

    debug!("Obtain Full Diagnostics: {:?}", start.elapsed().as_secs_f64());
  }


  // --| Updated diagnostics ----------
  pub async fn update_diagnostics(&self) {
    let urls = self.get_urls().await;

    debug!("Update Diagnostics");
    let docs = &self.docs.lock().await;   

    for url in urls {
      let doc = docs.get(&url).unwrap();
      let context = doc.get_content();
      self.obtain_full_diagnostics(url.clone(), context.to_string()).await;
    }
  }

  // --| Change Events -------------------------- 
  // --|-----------------------------------------
  // --| did_open handler -------------
  pub async fn on_open(&self, params: DidOpenTextDocumentParams) {
    let start = Instant::now();

    let docs = &mut self.docs.lock().await; 

    let mut parser = self.parser.lock().await;
    let parse_tree = &mut self.parse_tree.lock().await;

    let document = FullTextDocument::from_params(&params, &mut parser);
    docs.insert(document.uri.clone(), document.clone());
    if let Some(tree) = document.tree {
      parse_tree.insert(document.uri.clone(), tree.clone());
      debug!("{}", TreeWrapper(tree));
    } 
      // debug!("Begin Publishing Diagnostics: {:?}", uri.clone());
      // self.publish_diagnostics(uri.clone(), content.to_string()).await;

      // debug!("Diagnostic Published: {:?}", uri.clone());
    debug!("File Opened: {}ms", start.elapsed().as_secs_f64());
    self.client.log_message(MessageType::INFO, format!("file opened: {:?}", document.uri)).await;
    // }
  }

  // --| onChange event handler -------
  pub async fn on_change(&self, params: DidChangeTextDocumentParams) {
    if params.content_changes.is_empty() { return; }
    let start = Instant::now();

    if let Some(document) = self.docs.lock().await.get_mut(&params.text_document.uri) {
      let mut parser = self.parser.lock().await;
      let mut parse_tree = self.parse_tree.lock().await;
      let changes: Vec<TextDocumentContentChangeEvent> = params.content_changes.into_iter()
        .map(|change| {
          let range = change.range.map(|range| {
            get_range(
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

      for change in changes {
        let edits = &get_tree_edits(&change, document, version as i64);
        if let Some(edits) = edits { tree.edit(edits); }
      }

      let level = &self.log_data.lock().await;
      let new_tree: Tree;
      let content = document.rope.to_string();
      let uri = params.text_document.uri.clone();

      if level.log_level == LevelFilter::DEBUG {
        new_tree = parser.parse(&content, Some(tree)).unwrap();
        let old_tree = parse_tree.insert(uri.clone(), new_tree.clone());

        if level.verbose {
          debug!("{}", TreeWrapper(old_tree.unwrap().clone()));
          debug!("{}", TreeWrapper(new_tree.clone()));
        }

        debug!("Verbose: {}", level.verbose);

        debug!("Incremental updating: {}ms", start.elapsed().as_secs_f64());
      } else{
        new_tree = parser.parse(&content, Some(tree)).unwrap();
        parse_tree.insert(uri.clone(), new_tree.clone());
      } 

      if !new_tree.root_node().has_error() {
        self.publish_diagnostics(params.text_document.uri.clone(), None).await;
      } else {
        self.obtain_basic_diagnostics(uri, content , new_tree).await;
      }
    }
  }

  // --| didSave handler -------------
  pub async fn on_save(&self, params: DidSaveTextDocumentParams) {
    let content = params.text;
    let uri = params.text_document.uri;

    if let Some(text) = content {
      debug!("Begin Publishing Diagnostics: {:?}", uri.clone());
      self.obtain_full_diagnostics(uri.clone(), text.to_string()).await;
    }
    else{
      error!("Failed to get document content: {:?}", uri);
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

    debug!("Removing Document: {:?}", uri);
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

    debug!("Completion Requested: {:?}", params);

    if params.context.is_some() {
      let uri = params.text_document_position.text_document.uri;

      let tmp = &mut self.docs.lock().await;
      let doc_tmp = tmp.get_mut(&uri).unwrap();

      let doc_data = doc_tmp.get_content();
      if doc_data.len() == 0 { debug!("Completion: No document found"); return Ok(None); }

      debug!("Context is Some() requesting getcomplete({:?}, {:?}, {:?})", &self.client, location, uri.path());

      match Some(doc_data) {
        Some(context) => Ok(completions::get_completion(context, location, &self.client, uri.path()).await),
        None => { debug!("No document? Content was None"); Ok(None) }
      }
    } else {
      debug!("No document? Content was None");
      Ok(None)
    }
  }

  // --| Hover Handler ----------------
  pub async fn on_hover(&self, params: HoverParams) -> Result<Option<Hover>> {
    debug!("Hover Requested: {:?}", &params);

    let position = params.text_document_position_params.position;
    let uri = params.text_document_position_params.text_document.uri;

    let tmp = &mut self.docs.lock().await;
    let doc_tmp = tmp.get_mut(&uri).unwrap();

    let doc_data = doc_tmp.get_content();

    self.client.log_message(MessageType::INFO, "Hovered!").await;

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
        debug!("Hover: Parser Loaded");

        let ts_tree = parser.parse(context.clone(), None);
        let tree = ts_tree.unwrap();

        debug!("Hover: Looking up token at position: {:?} ctx: {:?} tree: {:?}", position, context, tree.root_node());
        let lsp_action = "hover".to_string();
        let output = get_from_position(position, tree.root_node(), context, lsp_action);
        if output.is_none() { debug!("Hover: No token found"); }

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

  // --| Execute Command Handler ------
  pub async fn on_execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
    debug!("Execute Command Requested: {:?}", &params);

    let command = params.command;
    let _args = &params.arguments;

    match command.as_str() {
      "cyberls.toggle_verbose" => {
        let mut log_data = self.log_data.lock().await;
        log_data.verbose = !log_data.verbose;

        debug!("Verbose: {}", log_data.verbose);
        self.client.log_message(MessageType::INFO, format!("Verbose: {}", log_data.verbose)).await;
      },
      // "cyberls.loglevel" => {
      //   let debug = &mut self.log_data.log_level.clone();
      //   // *debug = LevelFilter::try_from(args);
      //   let command_params = match serde_json::value::from_value(args[0].clone()) {
      //     Ok(value) => value,
      //     Err(err) => {}
      //   };
      //   *debug = LevelFilter::from(command_params);
      //
      //   self.client.log_message(MessageType::INFO, format!("Debug: {}", *debug)).await;
      // },
      _ => {
        self.client.log_message(MessageType::ERROR, format!("Unknown command: {}", command)).await;
      }
    }
    Ok(None)
  }
}
