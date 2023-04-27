use lsp_types::{Url, TextDocumentContentChangeEvent};
use serde_derive::{Deserialize, Serialize};
use tracing_subscriber::filter;

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

// --| Debug Structures ----------
// --|----------------------------
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
  pub enable: bool,
  pub verbose: bool,
  pub show_warnings: bool,
  pub unstable_features: bool,
  pub performance_metrics: bool,
  pub root_dir: Option<String>,
  pub log_level: Option<String>,
  pub client_name: Option<String>,
}

impl Config {
  pub fn default() -> Config {
    Config {
      enable: true,
      verbose:false, 
      root_dir: None,
      log_level: None,
      client_name: None,
      show_warnings: true,
      unstable_features: false,
      performance_metrics: false,
    }
  }
}

#[derive(Debug)]
pub struct LogData {
  pub(crate) log_level: filter::LevelFilter,
  pub(crate) verbose: bool,
}

impl Default for LogData {
fn default() -> Self {
    LogData { log_level: filter::LevelFilter::WARN, verbose: false }
  }
}

impl LogData {
  pub fn new(log_level: filter::LevelFilter, verbose: bool) -> Self {
    Self { log_level, verbose }
  }
}
