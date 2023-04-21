use serde_json::Value;
use std::collections::BTreeMap;
use lsp_types::{
  notification::{ DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification, },
  DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, Position,
  Range, TextDocumentContentChangeEvent, Url
};

use super::FullTextDocument;

// --| Text Document Container -------------
// --|--------------------------------------
//region TextDocumentContainer
#[derive(Default, Debug, Clone)]
pub struct TextDocuments(BTreeMap<Url, FullTextDocument>);

impl TextDocuments {
  /// Create a text documents
  ///
  /// # Examples
  ///
  /// Basic usage:
  ///
  /// ```
  /// use lsp_textdocument::TextDocuments;
  ///
  /// let text_documents = TextDocuments::new();
  /// ```
  pub fn new() -> Self {
    tracing::info!("TextDocuments::new");
    Self(BTreeMap::new())
  }

  pub fn documents(&self) -> &BTreeMap<Url, FullTextDocument> {
    &self.0
  }

  /// Get specify document by giving Url
  ///
  /// # Examples:
  ///
  /// Basic usage:
  /// ```
  /// use lsp_textdocument::TextDocuments;
  /// use lsp_types::Url;
  ///
  /// let text_documents = TextDocuments::new();
  /// let uri:Url = "file://example.txt".parse().unwrap();
  /// text_documents.get_document(&uri);
  /// ```
  pub fn get_document(&self, uri: &Url) -> Option<&FullTextDocument> {
    self.0.get(uri)
  }

  /// Get specify document content by giving Range
  ///
  /// # Examples
  ///
  /// Basic usage:
  /// ```no_run
  /// use lsp_textdocument::TextDocuments;
  /// use lsp_types::{Url, Range, Position};
  ///
  /// let uri: Url = "file://example.txt".parse().unwrap();
  /// let text_documents = TextDocuments::new();
  ///
  /// // get document all content
  /// let content = text_documents.get_document_content(&uri, None);
  /// assert_eq!(content, Some("hello rust!"));
  ///
  /// // get document specify content by range
  /// let (start, end) = (Position::new(0, 1), Position::new(0, 9));
  /// let range = Range::new(start, end);
  /// let sub_content = text_documents.get_document_content(&uri, Some(range));
  /// assert_eq!(sub_content, Some("ello rus"));
  /// ```
  pub fn get_document_content(&self, uri: &Url, _range: Option<Range>) -> Option<&str> {
    self.0.get(uri).map(|document| document.get_text())
  }

  /// Get specify document's language by giving Url
  ///
  /// # Examples
  ///
  /// Basic usage:
  /// ```no_run
  /// use lsp_textdocument::TextDocuments;
  /// use lsp_types::Url;
  ///
  /// let text_documents = TextDocuments::new();
  /// let uri:Url = "file://example.js".parse().unwrap();
  /// let language =  text_documents.get_document_language(&uri);
  /// assert_eq!(language, Some("javascript"));
  /// ```
  pub fn get_document_language(&self, uri: &Url) -> Option<&str> {
    self.0.get(uri).map(|document| document.language_id.as_str())
  }

  /// Listening the notification from client, you just need to pass `method` and `params`
  ///
  /// # Examples:
  ///
  /// Basic usage:
  /// ```no_run
  /// use lsp_textdocument::TextDocuments;
  ///
  /// let method = "textDocument/didOpen";
  /// let params = serde_json::to_value("message produced by client").unwrap();
  ///
  /// let mut text_documents = TextDocuments::new();
  /// text_documents.listen(method, &params);
  /// ```
  pub fn listen(&mut self, method: &str, params: &Value) -> bool {
    match method {
      DidOpenTextDocument::METHOD => {
        let params: DidOpenTextDocumentParams = serde_json::from_value(params.clone()) .expect("Expect receive DidOpenTextDocumentParams");
        let text_document = params.text_document;

        let document = FullTextDocument::new(
          text_document.uri.clone(),
          text_document.language_id,
          text_document.version.into(),
          text_document.text,
        );
        
        tracing::info!("DidOpenTextDocumentParams: {:?}, Document: {:?} ", &document.uri, &document);

        if self.0.contains_key(&document.uri) {
          tracing::info!("Document already exists: {:?}", &document.uri);
          return false;
        }

        let result = self.0.insert(document.uri.clone(), document.clone());

        if !result.is_none() {
          tracing::info!("Document insert failed: {:?}", &document.uri);
          return false;
        } else {
          tracing::info!("Document insert success: {:?}", &document.uri);

          let docs = &self.documents();
          let urls = docs.keys();

          for url in urls { tracing::info!("Document url: {:?} data: {:?}", url, docs.get(url)); }
        }
        true
      }

      DidChangeTextDocument::METHOD => {
        let params: DidChangeTextDocumentParams = serde_json::from_value(params.clone()).expect("Expect receive DidChangeTextDocumentParams");

        if let Some(document) = self.0.get_mut(&params.text_document.uri) {
          let changes = &params.content_changes;
          let version = params.text_document.version;
          document.update(changes.to_vec(), version.into());
          tracing::info!("DidChangeTextDocumentParams: {:?}, Document: {:?} ", &params.text_document.uri, &document);
        } else {
          tracing::info!("Document not found: {:?}", &params.text_document.uri);
        }
        
        true
      }

      DidCloseTextDocument::METHOD => {
        let params: DidCloseTextDocumentParams = serde_json::from_value(params.clone()).expect("Expect receive DidCloseTextDocumentParams");

        tracing::info!("DidCloseTextDocument: {:?}", &params.text_document.uri);
        self.0.remove(&params.text_document.uri);
        true
      }
      // Else ignore
      _ => { false }
    }
  }
}
//endregion
