use ropey::Rope;
use serde_json::Value;
use std::collections::BTreeMap;
use lsp_types::{
  notification::{ DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification, },
  DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, Position,
  Range, TextDocumentContentChangeEvent, Url
};

// --| Text Document -------------
// --|----------------------------
#[derive(Clone, Debug)]
pub struct FullTextDocument {
  /// The text document's URI.
  pub uri: Url,

  /// The text document's language identifier.
  pub language_id: String,

  /// The version number of this document. 
  pub version: i64,

  /// The content of the opened text document.
  pub text: String,

  line_offset: Option<Vec<usize>>,
  pub rope: Rope,
}

// --| Print Implementation ------
impl std::fmt::Display for FullTextDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!( f, "FullTextDocument {{ uri: {}, language_id: {}, version: {}, text: {}, line_offset: {:?}, rope: {} }}",
              self.uri, self.language_id, self.version, self.text, self.line_offset, self.rope)
    }
}

impl FullTextDocument {
  /// Creates a new `FullTextDocument`.
  pub fn new(uri: Url, language_id: String, version: i64, text: String) -> FullTextDocument {
    FullTextDocument {
      uri,
      language_id,
      version,
      text: text.clone(),
      line_offset: None,
      rope: Rope::from_str(&text),
    }
  }

  /// Updates the `FullTextDocument` with the given changes.
  pub fn update(&mut self, changes: Vec<TextDocumentContentChangeEvent>, version: i64) {

    for change in changes {
      if Self::is_incremental(&change) {
        let range = get_wellformed_range(change.range.unwrap());

        let start_offset = self.offset_at(range.start);
        let end_offset = self.offset_at(range.end);

        self.rope.remove(start_offset..end_offset);
        self.rope.insert(start_offset, &change.text);

      } else if Self::is_full(&change) {
        self.text = change.text;
        self.line_offset = None;
      }
      self.version = version;
    }
  }

  /// Updates the `FullTextDocument` with the given changes and returns the full text.
  pub fn update_full(&mut self, changes: Vec<TextDocumentContentChangeEvent>, version: i64) -> FullTextDocument {

    for change in changes {
      if Self::is_incremental(&change) {
        let range = get_wellformed_range(change.range.unwrap());

        let start_offset = self.offset_at(range.start);
        let end_offset = self.offset_at(range.end);

        self.rope.remove(start_offset..end_offset);
        self.rope.insert(start_offset, &change.text);

      } else if Self::is_full(&change) {
        self.text = change.text;
        self.line_offset = None;
      }
      self.version = version;
    }

    self.clone()
  }

  /// Returns the byte offset from the given start and end offset.
  pub fn transform_offset_to_byte_offset( &self, start_offset: usize, end_offset: usize,) -> (usize, usize) {
    let start_byte = self.text.chars()
      .take(start_offset)
      .fold(0, |acc, cur| acc + cur.len_utf8());

    let end_byte = self.text.chars().skip(start_offset)
      .take(end_offset - start_offset)
      .fold(0, |acc, cur| acc + cur.len_utf8()) + start_byte;

    (start_byte, end_byte)
  }

  /// Returns the line [Position] for the given offset.
  pub fn position_at(&mut self, mut offset: u32) -> Position {
    offset = offset.min(self.text.len() as u32).max(0);

    let line_offsets = self.get_line_offsets();
    let mut low = 0usize;
    let mut high = line_offsets.len();

    if high == 0 {
      return Position { line: 0, character: offset, };
    }
    while low < high {
      let mid = low + (high - low) / 2;

      if line_offsets[mid] as u32 > offset { high = mid; }
      else { low = mid + 1; }
    }

    let line = low as u32 - 1;
    return Position { line, character: offset - line_offsets[line as usize] as u32, };
  }

  /// Returns the number of lines in the document.
  pub fn line_count(&mut self) -> usize {
    self.rope.len_lines()
  }

  /// Determines if the change is incremental by checking if the range is set.
  pub fn is_incremental(event: &TextDocumentContentChangeEvent) -> bool {
    event.range.is_some()
  }

  /// Determines if the change is full by checking if the range and range length are not set.
  pub fn is_full(event: &TextDocumentContentChangeEvent) -> bool {
    !event.range_length.is_some() && !event.range.is_some()
  }

  /// Returns the line offsets
  pub fn get_line_offsets(&mut self) -> &mut Vec<usize> {
    if self.line_offset.is_none() {
      self.line_offset = Some(compute_line_offsets(&self.text, true, None));
    }
    self.line_offset.as_mut().unwrap()
  }

  /// Returns the full text of the document if exists. Otherwise, returns an empty string.
  pub fn get_text(&self) -> &str {
    let end_char = self.rope.len_chars();
    return self.rope.slice(0..end_char).as_str().unwrap_or("")
  }

  pub fn offset_at(&mut self, position: Position) -> usize {
    let Position { line, character } = position;
    if position.line >= self.line_count() as u32 {
      return self.rope.len_chars();
    }

    let line_offset = self.rope.line_to_char(line as usize);

    let next_line_offset = if position.line + 1 < self.line_count() as u32 {
      self.rope.line_to_char(line as usize + 1)
    } else {
      self.rope.len_chars()
    };

    (line_offset + character as usize)
      .min(next_line_offset)
      .max(line_offset)
  }
}

pub fn compute_line_offsets(text: &String, is_at_line_start: bool, text_offset: Option<usize>,) -> Vec<usize> {
  let text_offset = if let Some(offset) = text_offset { offset }
  else { 0 };

  let mut result = if is_at_line_start { vec![text_offset] }
  else { vec![] };

  let char_array: Vec<char> = text.chars().collect();
  let mut i = 0;

  while i < char_array.len() {
    let &ch = unsafe { char_array.get_unchecked(i) };

    if ch == '\r' || ch == '\n' {
      if ch == '\r' && i + 1 < char_array.len() && unsafe { char_array.get_unchecked(i + 1) == &'\n' } {
        i += 1;
      }
      result.push(text_offset + i + 1);
    }
    i += 1;
  }
  result
}

fn get_wellformed_range(range: Range) -> Range {
  let start = range.start;
  let end = range.end;

  if start.line > end.line || (start.line == end.line && start.character > end.character) {
    Range::new(end, start)
  }
  else { range }
}

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
