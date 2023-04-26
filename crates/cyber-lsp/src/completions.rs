mod scanner;

// use crate::CompletionResponse;
use std::path::{Path, PathBuf};
use crate::utils::treehelper::{get_pos_type, PositionType, get_from_position};
use lsp_types::{CompletionItem, CompletionItemKind, MessageType, Position, CompletionResponse};
use tracing::info;

/// get the completion messages
pub async fn get_completion(source: &str, location: Position, client: &tower_lsp::Client, local_path: &str,) -> Option<CompletionResponse> {
    let lsp_action = "completion".to_string();  

    info!("Loading tree-sitter-cyber parser...");
    let mut parser = cyber_tree_sitter::try_init_parser().expect("Parser failed to load");

    let ts_tree = parser.parse(source, None);
    let tree = ts_tree.unwrap();
    
    let mut complete: Vec<CompletionItem> = vec![];
    info!("Loading tree-sitter-cyber parser...done");

    // --| Load Completion Items ----------
    let type_data = get_from_position(location, tree.root_node(), source, lsp_action);

    if let Some(type_data) = type_data {
      info!("KeywordDetail: {:?} Desc: {:?}", type_data.keyword, type_data.description); 
    } else {
      info!("KeywordDetail: None");
    }

    let pos_type = get_pos_type(location, tree.root_node(), source, PositionType::NotFind);

    if let Some(mut message) = get_nested_completion(tree.root_node(), source, Path::new(local_path), pos_type, Some(location),){ 
      complete.append(&mut message); 
    }

    if complete.is_empty() {
        client.log_message(MessageType::INFO, "Empty").await;
        None
    } else {
        Some(CompletionResponse::Array(complete))
    }
}
/// get the variable from the loop
/// use position to make only can complete which has show before
fn get_nested_completion(input: tree_sitter::Node, source: &str, local_path: &Path, pos_type: PositionType, location: Option<Position>,) -> Option<Vec<CompletionItem>> {
    if let Some(location) = location {
        if input.start_position().row as u32 > location.line { return None; }
    }

    let source_array: Vec<&str> = source.lines().collect();
    let mut cursor = input.walk();
    let mut completion_item: Vec<CompletionItem> = vec![];

    for child in input.children(&mut cursor) {
        if let Some(location) = location {
            // Break if the child is on a different line
            if child.start_position().row as u32 > location.line { break; }
        }

        match child.kind() {
            "function_definition" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &source_array[h][x..y];
                completion_item.push(CompletionItem {
                    label: format!("{name}()"),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!(
                        "defined function\nfrom: {}",
                        local_path.file_name().unwrap().to_str().unwrap()
                    )),
                    ..Default::default()
                });
            }
            
            "if_condition" | "for_range_loop" | "for_iterable_loop" => {
                if let Some(mut message) =
                    get_nested_completion(child, source, local_path, pos_type, location)
                {
                    completion_item.append(&mut message);
                }
            }
            _ => {}
        }
    }
    if completion_item.is_empty() {
        None
    } else {
        Some(completion_item)
    }
}

fn cyber_try_exists(input: &PathBuf) -> std::io::Result<bool> {
    match std::fs::metadata(input) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

