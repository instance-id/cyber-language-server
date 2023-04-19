use cyber_tree_sitter as tree_sitter;
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::COMMENT,        // 0
            SemanticTokenType::KEYWORD,        // 1
            SemanticTokenType::NAMESPACE,      // 2
            SemanticTokenType::TYPE,           // 3
            SemanticTokenType::TYPE_PARAMETER, // 4
            SemanticTokenType::ENUM_MEMBER,    // 5
            SemanticTokenType::STRING,         // 6
            SemanticTokenType::NUMBER,         // 7
            SemanticTokenType::MACRO,          // 8
        ],
        token_modifiers: vec![
            // TODO
        ],
    }
}

#[derive(Debug, Clone, Copy)]
enum TokenType {
    // Keep these in sync with indices of `token_types` above!
    Comment = 0,
    Keyword = 1,
    Namespace = 2,
    Type = 3,
    TypeVariable = 4,
    Constructor = 5,
    String = 6,
    Number = 7,
    Special = 8,
}

impl std::convert::TryFrom<cyber_highlight::TokenType> for TokenType {
    type Error = cyber_highlight::TokenType;
    fn try_from(tt: cyber_highlight::TokenType) -> Result<Self, Self::Error> {
        match tt {
            cyber_highlight::TokenType::Comment => Ok(Self::Comment),
            cyber_highlight::TokenType::Bracket => Err(tt),
            cyber_highlight::TokenType::Delimiter => Err(tt),
            cyber_highlight::TokenType::KeywordImport => Ok(Self::Keyword),
            cyber_highlight::TokenType::Keyword => Ok(Self::Keyword),
            cyber_highlight::TokenType::KeywordReturn => Ok(Self::Keyword),
            cyber_highlight::TokenType::KeywordConditional => Ok(Self::Keyword),
            cyber_highlight::TokenType::Symbol => Ok(Self::Special),
            cyber_highlight::TokenType::Namespace => Ok(Self::Namespace),
            cyber_highlight::TokenType::Type => Ok(Self::Type),
            cyber_highlight::TokenType::TypeVariable => Ok(Self::TypeVariable),
            cyber_highlight::TokenType::EnumMember => Ok(Self::Constructor),
            cyber_highlight::TokenType::TopLevelName => Err(tt),
            cyber_highlight::TokenType::Variable => Err(tt),
            cyber_highlight::TokenType::Operator => Err(tt),
            cyber_highlight::TokenType::String => Ok(Self::String),
            cyber_highlight::TokenType::Int => Ok(Self::Number),
            cyber_highlight::TokenType::Float => Ok(Self::Number),
            cyber_highlight::TokenType::Boolean => Err(tt),
            cyber_highlight::TokenType::Builtin => Err(tt),
        }
    }
}

pub fn get_tokens(
    tree: &tree_sitter::Tree,
    source: &str,
    query: &cyber_highlight::Query,
) -> SemanticTokens {
    let tokens = cyber_highlight::get_tokens(source, tree, query);
    let mut tokens_builder = TokensBuilder::new();
    for token in tokens {
        if let Ok(token_type) = token.token_type.try_into() {
            tokens_builder.push_node(token.node, token_type)
        }
    }
    SemanticTokens {
        result_id: None,
        data: tokens_builder.into_tokens().unwrap_or_default(),
    }
}

struct TokensBuilder(Vec<Node>);

#[derive(Debug)]
struct Node {
    start_line: usize,
    start_col: usize,
    token_type: TokenType,
    length: usize,
}

impl TokensBuilder {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn push_node(&mut self, node: tree_sitter::Node, token_type: TokenType) {
        let tree_sitter::Point { row, column } = node.start_position();
        let length = node.byte_range().len();
        self.0.push(Node {
            start_line: row,
            start_col: column,
            length,
            token_type,
        })
    }

    fn into_tokens(mut self) -> Option<Vec<SemanticToken>> {
        let mut tokens = Vec::new();
        self.0.sort_by_key(|node| (node.start_line, node.start_col));
        let mut current_line = 0;
        let mut current_col = 0;
        for node in self.0 {
            let delta_line: u32 = (node.start_line - current_line).try_into().ok()?;
            let delta_start: u32 = if delta_line > 0 {
                node.start_col.try_into().ok()?
            } else {
                (node.start_col - current_col).try_into().ok()?
            };
            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                token_type: node.token_type as u32,
                token_modifiers_bitset: 0,
                length: node.length as u32,
            });
            current_line = node.start_line;
            current_col = node.start_col;
        }
        Some(tokens)
    }
}
