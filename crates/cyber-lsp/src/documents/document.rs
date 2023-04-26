use cyber_tree_sitter::{Tree, Parser};
use ropey::Rope;
use lsp_types::{ Position, Range, TextDocumentContentChangeEvent, Url, DidOpenTextDocumentParams };

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

  /// The tree-sitter tree of the opened text document.
  pub tree: Option<Tree>,

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
  pub fn from_params(params: &DidOpenTextDocumentParams, parser: &mut Parser) -> FullTextDocument {
    let text = params.text_document.text.clone();

    FullTextDocument {
      language_id: params.text_document.language_id.clone(),
      version: params.text_document.version.into(),
      text: params.text_document.text.clone(),
      uri: params.text_document.uri.clone(),
      tree: parser.parse(&text, None),
      rope: Rope::from_str(&text),
      line_offset: None,
    }
  }

  pub fn new(uri: Url, language_id: String, version: i64, text: String) -> FullTextDocument {
    FullTextDocument {
      uri, language_id, version,
      text: text.clone(), tree: None,
      line_offset: None, rope: Rope::from_str(&text),
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
        self.rope = Rope::from_str(&self.text);
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
  pub fn get_content(&self) -> &str {
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


