use std::path::Path;

datatest_stable::harness!(
    tree_sitter_parser,
    "..",
    r"cyberls.*(cyber|stdin)$",
);

fn tree_sitter_parser(path: &Path) -> datatest_stable::Result<()> {
    let source = std::fs::read_to_string(path)?;
    let parsed = cyber_tree_sitter::init_parser()
        .parse(source, None)
        .unwrap();
    let root_node = parsed.root_node();
    assert!(!root_node.has_error());
    Ok(())
}
