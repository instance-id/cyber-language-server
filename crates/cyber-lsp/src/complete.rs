// use lsp_types::{CompletionItem, CompletionItemKind, CompletionResponse};

// --| Error Checking Placeholder ------
// pub fn checkerror(
//   input: tree_sitter::Node,
//   ) -> Option<Vec<(tree_sitter::Point, tree_sitter::Point)>> {
//   if input.has_error() {
//     if input.is_error() {
//       Some(vec![(input.start_position(), input.end_position())])
//     } else {
//       let mut course = input.walk();
//       {
//         let mut output = vec![];
//         for node in input.children(&mut course) {
//           if let Some(mut tran) = checkerror(node) {
//             output.append(&mut tran);
//           }
//         }
//         if output.is_empty() { None }
//         else { Some(output) }
//       }
//     }
//   } else {
//     None
//   }
// }

// pub fn getcomplete(input: tree_sitter::Node, source: &str, id: &str) -> Option<CompletionResponse> {
//     let source_array: Vec<&str> = source.lines().collect();
//     let mut cursor = input.walk();
//     let mut hasid = false;
//     let mut complete: Vec<CompletionItem> = vec![];
//     for child in input.children(&mut cursor) {
//         match child.kind() {
//             "something" => {
//                 let h = child.start_position().row;
//                 let ids = child.child(2).unwrap();
//                 let x = ids.start_position().column;
//                 let y = ids.end_position().column;
//                 let name = &newsource[h][x..y];
//                 println!("name= {}", name);
//                 if name == id {
//                     println!("test");
//                     hasid = true;
//                 } else {
//                     hasid = false;
//                 }
//             }
//             "meh" => {
//                 let h = child.start_position().row;
//                 let ids = child.child(0).unwrap();
//                 let x = ids.start_position().column;
//                 let y = ids.end_position().column;
//                 let name = &newsource[h][x..y];
//                 complete.push(CompletionItem {
//                     label: name.to_string(),
//                     kind: Some(CompletionItemKind::VALUE),
//                     detail: Some("message".to_string()),
//                     ..Default::default()
//                 });
//             }
//             "function" => {
//                 let h = child.start_position().row;
//                 let ids = child.child(1).unwrap();
//                 let x = ids.start_position().column;
//                 let y = ids.end_position().column;
//                 let name = &newsource[h][x..y];
//                 complete.push(CompletionItem {
//                     label: name.to_string(),
//                     kind: Some(CompletionItemKind::FUNCTION),
//                     detail: Some("message".to_string()),
//                     ..Default::default()
//                 });
//             }
//             "other" => {
//                 let output = getcomplete(child, source, id);
//                 if output.is_some() {
//                     return output;
//                 }
//             }
//             _ => {}
//         }
//     }
//     if hasid {
//         Some(CompletionResponse::Array(complete))
//     } else {
//         None
//     }
// }

// #[cfg(test)]
// mod grammertests {
//   #[test]
//   fn test_complete() {
//     let source = "put some source here";
//     let mut parse = tree_sitter::Parser::new();
//     parse.set_language(tree_sitter_cyber::language()).unwrap();
//     let tree = parse.parse(source, None).unwrap();
//     let root = tree.root_node();
//     println!("{}", root.to_sexp());
//     let a = super::getcomplete(root, source, "window");
//     println!("{:#?}", a);
//   }
// }
