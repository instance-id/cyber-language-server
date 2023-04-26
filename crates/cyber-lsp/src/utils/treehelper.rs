use std::collections::HashMap;

use cyber_tree_sitter::InputEdit;
use cyber_tree_sitter::Tree;
use lsp_types::Range;
use lsp_types::Position;
use lsp_types::TextDocumentContentChangeEvent;
use once_cell::sync::Lazy;
use tracing::debug;
use tracing::error;
use tracing::info;
use tree_sitter::{ Node, Point, Query, QueryCursor };

use crate::datatypes::*;
use super::loader::LANGUAGE_DOCS;
use crate::documents::FullTextDocument;

macro_rules! query_for_nodes {
  ($query:expr,$node:expr,$source:expr) => {
    QueryCursor::new()
      .captures(
        &Query::new(cyber_tree_sitter::cyber_language(), $query).expect("unable to create query"),
        $node, $source.as_bytes())
      .map(|x| x.0.captures).flatten()
      .map(|c| c.node)
  };
}

macro_rules! query_for_ranges {
  ($query:expr,$node:expr,$source:expr) => {
    query_for_nodes!($query, $node, $source).map(|node| {
      let start = node.start_position();
      let end = node.end_position();

      Range {
        start: Position { line: start.row as u32, character: start.column as u32, },
        end: Position { line: end.row as u32, character: end.column as u32, },
      }
    })
  };
}

pub fn get_range(start_row: u32, start_column: u32, end_row: u32, end_column: u32,) -> Range {
    Range::new(
        Position::new(start_row, start_column),
        Position::new(end_row, end_column),
    )
}

/// Converts [tree_sitter] Point to [lsp_types] Position
/// treesitter to lsp_types
#[inline]
pub fn point_to_position(input: Point) -> Position {
  Position { line: input.row as u32, character: input.column as u32 }
}

/// Converts [lsp_types] Position to [tree_sitter] Point
#[inline]
pub fn position_to_point(input: Position) -> Point {
  Point { row: input.line as usize, column: input.character as usize }
}

/// Search the documentation store for the relevant keyword details for the given position
pub fn get_from_position(location: Position, root: Node, source: &str, lsp_action: String) -> Option<KeywordDetail> {
  match (get_string_at_pos(location, root, source), get_pos_type(location, root, source, PositionType::NotFind)) {
    (Some(message), _) => {
      info!("Message: {}", message);

      let mut value = MESSAGE_STORAGE.get(&lsp_action);
      if value.is_none() { value = MESSAGE_STORAGE.get(&lsp_action.to_lowercase()); }
     
     return value?.lookup(&message).map(|x| x.clone());
    }

    (None, _) => {
      info!("Message: None??"); None
    },
  }
}

/// Get string from current document the given position
pub fn get_string_at_pos(location: Position, root: Node, source: &str) -> Option<String> {
  let position = position_to_point(location);
  let source_array: Vec<&str> = source.lines().collect();
  
  let mut cursor = root.walk();

  for child in root.children(&mut cursor) {
    if position.row <= child.end_position().row && position.row >= child.start_position().row {
      if child.child_count() != 0 {
        let recurse_pos = get_string_at_pos(location, child, source);
        if recurse_pos.is_some() { return recurse_pos; };
      }

      else if child.start_position().row == child.end_position().row 
        && position.column <= child.end_position().column 
          && position.column >= child.start_position().column {
                          
        let h = child.start_position().row;
        let x = child.start_position().column;
        let y = child.end_position().column;

        let message = &source_array[h][x..y];
        return Some(message.to_string());
      }
    }
  }

  // No string found
  None
}

/// Get Tree Node range based on the current position
pub fn get_position_range(location: Position, root: Node) -> Option<Range> {
  let position = position_to_point(location);
  let mut cursor = root.walk();

  for child in root.children(&mut cursor){
    // if is inside same line
    if position.row <= child.end_position().row
      && position.row >= child.start_position().row
      {
        if child.child_count() != 0 {
          let child_pos = get_position_range(location, child);
          if child_pos.is_some() { return child_pos; }
        }
        // if is the same line
        else if child.start_position().row == child.end_position().row && 
          position.column <= child.end_position().column && 
            position.column >= child.start_position().column
        {
          return Some(Range {
            start: point_to_position(child.start_position()),
            end: point_to_position(child.end_position()),
          });
        }
      }
  }
  None
}

pub fn get_tree_edits(change: &TextDocumentContentChangeEvent, document: &mut FullTextDocument, version: i64,) -> Option<InputEdit> {
    if change.range.is_none() || change.range_length.is_none() { return None; }

    let range = change.range.unwrap();
    let start = range.start;
    let end = range.end;
    let start_char = document.rope.line_to_char(start.line as usize) + start.character as usize;
    let old_end_char = document.rope.line_to_char(end.line as usize) + end.character as usize;

    let start_byte = document.rope.char_to_byte(start_char);
    let old_end_byte = document.rope.char_to_byte(old_end_char);

    document.update(vec![change.clone()], version);
    let new_end_char = start_char + change.text.chars().count();
    let new_end_byte = document.rope.char_to_byte(new_end_char);

    let new_end_line = document.rope.char_to_line(new_end_char);
    let new_end_line_first_character = document.rope.line_to_char(new_end_line);
    let new_end_character = new_end_byte - new_end_line_first_character;

    Some(InputEdit {
        start_byte, old_end_byte, new_end_byte,
        start_position: Point::new(start.line as usize, start.character as usize),
        old_end_position: Point::new(end.line as usize, end.character as usize),
        new_end_position: Point::new(new_end_line, new_end_character),
    })
}

// --| Diagnostic Helpers -------------
// --|---------------------------------
// Here due to macros not being able to be used in other modules
pub fn get_parser_errors(source: &str, tree: Option<Tree>) -> Vec<Range> {
  match tree {
    Some(tree) => { 
      query_for_ranges!("(ERROR) @error", tree.root_node(), source).collect() 
    }
    None => {
      error!("tree was None when looking for parser errors");
      vec![Range {
        start: Position { line: 0, character: 0, },
        end: Position { line: 1, character: 0, },
      }]
    }
  }
}


// --| Language Definitions Storage ---
// --|---------------------------------
pub static MESSAGE_STORAGE: Lazy<HashMap<String, LanguageDefinition>> = Lazy::new(|| {
  let mut storage: HashMap<String, LanguageDefinition> = HashMap::new();
  info!("Loading language definitions from disk");

  // Load json files from disk and deserialize them into `datatypes::LanguageDefinitions`
  let language_docs: Vec<LanguageDoc>  = LANGUAGE_DOCS.clone().into_iter().map(|x| x.into()).collect();
  info!("Loaded {} language definitions", language_docs.len()); 

  for lang_doc in language_docs {
    let def_name = lang_doc.docname.to_string();
    info!("Loading language definition: {}", def_name);

    let lang_doc_json = std::fs::read_to_string(&lang_doc.path).expect("Failed to read file");

    let lang_defs: LanguageDefinition = match serde_json::from_str::<LanguageDefinition>(&lang_doc_json) {
        Ok(def) => { info!("Loaded language definition: {}", def_name); def },
        Err(err) => { error!("Failed to parse language definition: {}", err); continue; }
    };

    storage.insert(lang_defs.lsp_action.to_string(), lang_defs);
  } 
  storage
});

#[derive(Clone, Copy, Debug)]
pub enum PositionType {
  Variable,
  Comment,
  NotFind,
}

#[allow(unused)]
#[derive(Clone, Copy, Debug)]
pub enum LanguageConstruct {
  ControlFlow,
  Operator,
  Function,
  DataType,
  Variable,
  NotFind,
}

pub fn get_pos_type( location: Position, root: Node, source: &str, inputtype: PositionType,) -> PositionType {
  let position = position_to_point(location);
  let source_array: Vec<&str> = source.lines().collect();
  let mut cursor = root.walk();

  for child in root.children(&mut cursor) {
    if position.row <= child.end_position().row && position.row >= child.start_position().row
    {
      if child.child_count() != 0 {
        let _match_type = match child.kind() {
          "import_statement" | "assignment_statement" | "if_statement" => {
            let h = child.start_position().row;
            let ids = child.child(0).unwrap();
            let x = ids.start_position().column;
            let y = ids.end_position().column;
            let name = source_array[h][x..y].to_lowercase();
            
            debug!("name: {}", name);
            match name.as_str() { _ => PositionType::Variable, }
          }
          "normal_var" | "unquoted_argument" | "variable_def" | "variable" => {
            PositionType::Variable
          }

          "comment" => {
            debug!("Token Type: comment");  
            PositionType::Comment
          },

          _ => {
            let h = child.start_position().row;
            let ids = child.child(0).unwrap();
            let x = ids.start_position().column;
            let y = ids.end_position().column;
            let name = source_array[h][x..y].to_lowercase();

            debug!("name: {} kind: {}", name, child.kind());
            PositionType::Variable
          }
        };
      }
      // if is the same line
      else if child.start_position().row == child.end_position().row
        && position.column <= child.end_position().column
          && position.column >= child.start_position().column
          { return inputtype; }
    }
  }
  debug!("Returning None: {:?}", inputtype);
  PositionType::NotFind
}

#[derive(Debug, Clone)]
pub struct TreeWrapper(pub Tree);
impl std::fmt::Display for TreeWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty_display(f, self.0.root_node())?;
        Ok(())
    }
}

pub fn pretty_display(f: &mut std::fmt::Formatter<'_>, root: Node) -> std::fmt::Result {
    let mut stack = Vec::new();
    if !root.is_named() { return Ok(()); }

    writeln!(f, "\nSyntax Tree: Child Count: {}", &root.child_count())?;

    stack.push((root, 0));
    while let Some((node, level)) = stack.pop() {
        let kind = node.kind();
        let start = node.start_position();
        let end = node.end_position();
        writeln!(f, "{}{} [{}, {}] - [{}, {}] ", " ".repeat(level * 2), kind, start.row, start.column, end.row, end.column)?;

        for i in (0..node.named_child_count()).rev() {
            let child = node.named_child(i).unwrap();
            stack.push((child, level + 1));
        }
    }

    Ok(())
}
