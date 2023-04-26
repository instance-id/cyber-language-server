pub use tree_sitter::*;
pub use tree_sitter_cyber::{
  language as cyber_language, HIGHLIGHTS_QUERY as CYBER_HIGHLIGHTS_QUERY,
};

pub fn init_parser() -> tree_sitter::Parser {
  try_init_parser().unwrap_or_else(|lang_err| {
    panic!("Error initialising tree-sitter parser with cyber language: {}", lang_err)
  })
}

pub fn try_init_parser() -> Result<tree_sitter::Parser, tree_sitter::LanguageError> {
  let mut parser = tree_sitter::Parser::new();
  parser.set_language(cyber_language())?;
  Ok(parser)
}

pub fn get_language() -> Language {
  cyber_language()
}
