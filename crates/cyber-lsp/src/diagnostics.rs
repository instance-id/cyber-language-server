use std::fs;
use std::process::Command;
use std::path::{Path, PathBuf};
use cyber_tree_sitter::Point;
use lsp_types::DiagnosticSeverity;
use tracing::info;

/// Check for syntax errors. If there is error,
/// return the position of the error and message
#[derive(Default)]
pub struct ErrorInfo {
  pub entries: Vec<ErrorEntry>,
}

impl ErrorInfo {
  pub fn new() -> Self {
    Self { entries: vec![] }
  }

  pub fn combine(&mut self, other: &mut ErrorInfo) {
    self.entries.append(&mut other.entries);
  }

  pub fn add(&mut self, start: Point, end: Point, message: String, severity: Option<DiagnosticSeverity>) {
    self.entries.push(ErrorEntry { start, end, message, severity });
  }

  fn is_empty(&self) -> bool { self.entries.is_empty() }
}

pub struct ErrorEntry {
  pub start: Point,
  pub end: Point,
  pub message: String,
  pub severity: Option<DiagnosticSeverity>,
}


pub fn check_compile_error(local_path: &Path, _source: &str) -> Option<ErrorInfo> {
  let mut diag_result = ErrorInfo::new();
  let path_str = local_path.to_str().unwrap();

  let output = if cfg!(target_os = "windows") {
    Command::new("cmd")
      .args(["/C", &format!("cyber compile {:?}", path_str)]).output()
      .expect("failed to execute process")
  } else {
    Command::new("sh")
      .args(["-c", &format!("cyber compile {:?}", path_str)]).output()
      .expect("failed to execute process")
  };

  let results = output.stdout;
  let error = output.stderr;
  let error = String::from_utf8(error).unwrap();
  let results = String::from_utf8(results).unwrap();

  if !error.is_empty() {
    let err_lines: Vec<&str> = error.lines().collect();

    if err_lines[0].contains("Bytecode:") { return None; }
    else if err_lines[0].contains("ParseError:") {
      let err_msg = err_lines[0].split("ParseError: ").collect::<Vec<&str>>()[1];
      let err_row = err_lines[2].split(":").collect::<Vec<&str>>()[1].trim().parse::<usize>().unwrap();
      let err_col = err_lines[2].split(":").collect::<Vec<&str>>()[2].trim().parse::<usize>().unwrap();

      diag_result.add(
        tree_sitter::Point{ row: err_row - 1, column: err_col }, 
        tree_sitter::Point{ row: err_row - 1, column: err_col }, 
        err_msg.to_string(), 
        Some(DiagnosticSeverity::ERROR),
      );
    }
  }

  if !results.is_empty() { info!("Results: {}", results); }
  
  Some(diag_result)
}

pub fn check_tree_error(_local_path: &Path, source: &str, input: tree_sitter::Node) -> Option<ErrorInfo> {
  let _source_array: Vec<&str> = source.lines().collect();
  let mut error_info = ErrorInfo::new();

  if input.is_error() {
    error_info.add( input.start_position(), input.end_position(), "Grammar Error".to_string(), None);
    Some(error_info)
  } else {
    let _cursor = input.walk();
    {
      // --| Since I don't know what diagnostics to --
      // --| add but this is a possible example. -----

      // for node in input.children(&mut cursor) {
      //   if let Some(mut tran) = checkerror(local_path, source, node) {
      //     output.append(&mut tran.inner);
      //   }
      //
      //   if node.kind() == "TreeSitterNodeNameGoesHere" {
      //     let h = node.start_position().row;
      //     let ids = node.child(1).unwrap();
      //     let x = ids.start_position().column;
      //     let y = ids.end_position().column;
      //     let name = &source_array[h][x..y];
      //
      //     // --| Diagnostics processing here: -----
      //     if name.to_lowercase() == "something" && node.child_count() >= 5 {
      //       let mut walk = node.walk();
      //       let errors = crate::filewatcher::get_errors();
      //       for child in node.children(&mut walk) {
      //         let h = child.start_position().row;
      //         let x = child.start_position().column;
      //         let y = child.end_position().column;
      //
      //         if h < source_array.len() && y > x && y < newsource[h].len() {
      //           let name = &source_array[h][x..y];
      //
      //           if errors.contains(&name.to_string()) {
      //             output.push((
      //                 child.start_position(),
      //                 child.end_position(),
      //                 "Figure out and add some diagnostics".to_string(),
      //                 Some(DiagnosticSeverity::ERROR),
      //            ));
      //           }
      //         }
      //       }
      //     }
      //   }
      // }

      // --| Return with no diagnostic -- 
      // --| issues until I add some ----
      if error_info.is_empty() {
        None
      } else {
        let _err = ErrorInfo::new();
        Some(error_info)
      }
    }
  }
}

fn _cyber_try_exists(input: &PathBuf) -> std::io::Result<bool> {
  match std::fs::metadata(input) {
    Ok(_) => Ok(true),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
    Err(error) => Err(error),
  }
}

fn _scanner_include_error(path: &PathBuf) -> bool {
  match fs::read_to_string(path) {
    Ok(content) => {
      let mut parser = cyber_tree_sitter::try_init_parser().expect("Parser failed to load");
      
      let ts_tree = parser.parse(content, None);
      let tree = ts_tree.unwrap();

      tree.root_node().has_error()
    }
    Err(_) => true,
  }
}

