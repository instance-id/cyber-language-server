use lsp_types::{Url, TextDocumentContentChangeEvent};
use serde_derive::{Deserialize, Serialize};

pub(crate) struct TextDocumentItem {
  pub uri: Url,
  pub text: String,
  pub version: i32,
  pub changes: Vec<TextDocumentContentChangeEvent>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDoc {
  pub docname: String,
  pub path: String,
}

pub type _LanguageDefinitions = Vec<LanguageDefinition>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDefinition {
    pub lsp_action: String,                // Completion, Hover, Definition, etc.
    pub type_categories: Vec<TypeCategory>,   // ControlFlow, Operator, Function, DataType, Variable
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCategory {
    pub category: String,
    pub keywords: Vec<String>,
    pub keyword_details: Vec<KeywordDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordDetail {
    pub keyword: String,
    pub syntax: String,
    #[serde(rename = "type")]
    pub keyword_detail_type: String,
    pub node_type: Vec<String>,
    pub description: String,
    pub example: String,
}

impl LanguageDefinition {
  pub fn lookup(&self, keyword: &str) -> Option<&KeywordDetail> {
    self.type_categories.iter().find(|completion| completion.keywords.contains(&keyword.to_string()))
      .map(|completion| completion.keyword_details.iter().find(|keyword_detail| keyword_detail.keyword == keyword))
      .flatten()
  }

  pub(crate) fn _to_string(&self) -> String {
    todo!()
  }
}
