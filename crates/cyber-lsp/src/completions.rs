mod scanner;

// use crate::CompletionResponse;
use std::path::{Path, PathBuf};
use crate::utils::treehelper::{get_pos_type, PositionType, get_from_position};
use lsp_types::{CompletionItem, CompletionItemKind, MessageType, Position, CompletionResponse};
use tracing::info;

/// get the complet messages
pub async fn get_completion( source: &str, location: Position, client: &tower_lsp::Client, local_path: &str,) -> Option<CompletionResponse> {
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

    let postype = get_pos_type(location, tree.root_node(), source, PositionType::NotFind);

    if let Some(mut message) = getsubcomplete(tree.root_node(), source, Path::new(local_path), postype, Some(location),){ 
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
fn getsubcomplete( input: tree_sitter::Node, source: &str, local_path: &Path, postype: PositionType, location: Option<Position>,) -> Option<Vec<CompletionItem>> {
    if let Some(location) = location {
        if input.start_position().row as u32 > location.line {
            return None;
        }
    }

    let newsource: Vec<&str> = source.lines().collect();
    let mut course = input.walk();
    let mut complete: Vec<CompletionItem> = vec![];

    for child in input.children(&mut course) {
        if let Some(location) = location {
            if child.start_position().row as u32 > location.line {
                // if this child is below row, then break all loop
                break;
            }
        }

        match child.kind() {
            "function_def" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y];
                complete.push(CompletionItem {
                    label: format!("{name}()"),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!(
                        "defined function\nfrom: {}",
                        local_path.file_name().unwrap().to_str().unwrap()
                    )),
                    ..Default::default()
                });
            }
            
            "macro_def" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = &newsource[h][x..y];
                complete.push(CompletionItem {
                    label: format!("{name}()"),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!(
                        "defined function\nfrom: {}",
                        local_path.file_name().unwrap().to_str().unwrap()
                    )),
                    ..Default::default()
                });
            }
            
            "if_condition" | "foreach_loop" => {
                if let Some(mut message) =
                    getsubcomplete(child, source, local_path, postype, location)
                {
                    complete.append(&mut message);
                }
            }

            "normal_command" => {
                let h = child.start_position().row;
                let ids = child.child(0).unwrap();
                //let ids = ids.child(2).unwrap();
                let x = ids.start_position().column;
                let y = ids.end_position().column;
                let name = newsource[h][x..y].to_lowercase();

                if name == "include" && child.child_count() >= 3 {
                    let ids = child.child(2).unwrap();
                    if ids.start_position().row == ids.end_position().row {
                        let h = ids.start_position().row;
                        let x = ids.start_position().column;
                        let y = ids.end_position().column;
                        let name = &newsource[h][x..y];
                        if name.split('.').count() != 1 {
                            let subpath = local_path.parent().unwrap().join(name);
                            if let Ok(true) = cyber_try_exists(&subpath) {
                                if let Some(mut comps) =
                                    scanner::scanner_include_complete(&subpath, postype)
                                {
                                    complete.append(&mut comps);
                                }
                            }
                        }
                    }
                } 
            }
            _ => {}
        }
    }
    if complete.is_empty() {
        None
    } else {
        Some(complete)
    }
}

fn cyber_try_exists(input: &PathBuf) -> std::io::Result<bool> {
    match std::fs::metadata(input) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

