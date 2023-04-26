use crate::utils::treehelper::PositionType;

use super::get_nested_completion;
use lsp_types::CompletionItem;
use std::fs;
use std::path::PathBuf;

// Not used ...yet
pub fn scanner_include_complete(
    path: &PathBuf,
    postype: PositionType,
) -> Option<Vec<CompletionItem>> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let mut parser = cyber_tree_sitter::try_init_parser().expect("Parser failed to load");
            let thetree = parser.parse(content.clone(), None);
            let tree = thetree.unwrap();
            get_nested_completion(tree.root_node(), content.as_str(), path, postype, None)
        }
        Err(_) => None,
    }
}
