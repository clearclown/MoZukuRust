use anyhow::Result;
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use tower_lsp::lsp_types::{Position, SemanticToken};

/// Token information from morphological analysis
#[derive(Debug, Clone)]
pub struct TokenInfo {
    /// Surface form (表層形)
    pub surface: String,
    /// Part of speech (品詞)
    pub pos: String,
    /// Part of speech subcategory 1 (品詞細分類1)
    pub pos_detail1: String,
    /// Part of speech subcategory 2 (品詞細分類2)
    pub pos_detail2: String,
    /// Part of speech subcategory 3 (品詞細分類3)
    pub pos_detail3: String,
    /// Conjugation type (活用型)
    pub conjugation_type: String,
    /// Conjugation form (活用形)
    pub conjugation_form: String,
    /// Base form (基本形)
    pub base_form: String,
    /// Reading (読み)
    pub reading: String,
    /// Pronunciation (発音)
    pub pronunciation: String,
    /// Byte offset in text
    pub byte_offset: usize,
    /// Character offset in text
    pub char_offset: usize,
    /// Character length
    pub char_length: usize,
}

/// Morphological analyzer using Lindera
pub struct MorphologicalAnalyzer {
    tokenizer: Tokenizer,
}

impl MorphologicalAnalyzer {
    pub fn new() -> Result<Self> {
        let dictionary = load_dictionary("embedded://ipadic")?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        let tokenizer = Tokenizer::new(segmenter);
        Ok(Self { tokenizer })
    }

    /// Tokenize text and return token information
    pub fn tokenize(&self, text: &str) -> Vec<TokenInfo> {
        let mut tokens = match self.tokenizer.tokenize(text) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        let mut result = Vec::new();
        let mut char_offset = 0;

        for token in tokens.iter_mut() {
            let surface = token.surface.as_ref().to_string();
            let char_length = surface.chars().count();

            // Parse token details (IPADIC format)
            let details = token.details();

            let token_info = TokenInfo {
                surface: surface.clone(),
                pos: details.first().map(|s| s.to_string()).unwrap_or_else(|| "*".to_string()),
                pos_detail1: details.get(1).map(|s| s.to_string()).unwrap_or_else(|| "*".to_string()),
                pos_detail2: details.get(2).map(|s| s.to_string()).unwrap_or_else(|| "*".to_string()),
                pos_detail3: details.get(3).map(|s| s.to_string()).unwrap_or_else(|| "*".to_string()),
                conjugation_type: details.get(4).map(|s| s.to_string()).unwrap_or_else(|| "*".to_string()),
                conjugation_form: details.get(5).map(|s| s.to_string()).unwrap_or_else(|| "*".to_string()),
                base_form: details.get(6).map(|s| s.to_string()).unwrap_or_else(|| surface.clone()),
                reading: details.get(7).map(|s| s.to_string()).unwrap_or_default(),
                pronunciation: details.get(8).map(|s| s.to_string()).unwrap_or_default(),
                byte_offset: token.byte_start,
                char_offset,
                char_length,
            };

            result.push(token_info);
            char_offset += char_length;
        }

        result
    }

    /// Get hover information for a position in the text
    pub fn get_hover_info(&self, text: &str, position: Position) -> Option<String> {
        let tokens = self.tokenize(text);

        // Convert position to character offset
        let lines: Vec<&str> = text.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let mut char_offset = 0;
        for (i, line) in lines.iter().enumerate() {
            if i == position.line as usize {
                char_offset += position.character as usize;
                break;
            }
            char_offset += line.chars().count() + 1; // +1 for newline
        }

        // Find token at position
        for token in &tokens {
            let token_end = token.char_offset + token.char_length;
            if token.char_offset <= char_offset && char_offset < token_end {
                return Some(self.format_token_info(token));
            }
        }

        None
    }

    /// Format token information for hover display
    fn format_token_info(&self, token: &TokenInfo) -> String {
        let mut info = format!("## {}\n\n", token.surface);
        info.push_str(&format!("**品詞**: {}", token.pos));

        if token.pos_detail1 != "*" {
            info.push_str(&format!("-{}", token.pos_detail1));
        }
        if token.pos_detail2 != "*" {
            info.push_str(&format!("-{}", token.pos_detail2));
        }
        if token.pos_detail3 != "*" {
            info.push_str(&format!("-{}", token.pos_detail3));
        }
        info.push('\n');

        if token.base_form != "*" && token.base_form != token.surface {
            info.push_str(&format!("\n**基本形**: {}\n", token.base_form));
        }

        if token.conjugation_type != "*" {
            info.push_str(&format!("**活用型**: {}\n", token.conjugation_type));
        }

        if token.conjugation_form != "*" {
            info.push_str(&format!("**活用形**: {}\n", token.conjugation_form));
        }

        if !token.reading.is_empty() && token.reading != "*" {
            info.push_str(&format!("\n**読み**: {}\n", token.reading));
        }

        info
    }

    /// Get semantic tokens for syntax highlighting
    pub fn get_semantic_tokens(&self, text: &str) -> Vec<SemanticToken> {
        let tokens = self.tokenize(text);
        let mut semantic_tokens = Vec::new();

        let lines: Vec<&str> = text.lines().collect();
        let mut prev_line = 0u32;
        let mut prev_char = 0u32;

        for token in &tokens {
            // Convert char offset to line/column
            let (line, col) = self.char_offset_to_position(&lines, token.char_offset);

            // Calculate delta from previous token
            let delta_line = line - prev_line;
            let delta_start = if delta_line == 0 {
                col - prev_char
            } else {
                col
            };

            let token_type = self.pos_to_token_type(&token.pos);

            semantic_tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length: token.char_length as u32,
                token_type,
                token_modifiers_bitset: 0,
            });

            prev_line = line;
            prev_char = col;
        }

        semantic_tokens
    }

    /// Convert character offset to line/column position
    fn char_offset_to_position(&self, lines: &[&str], char_offset: usize) -> (u32, u32) {
        let mut current_offset = 0;

        for (line_num, line) in lines.iter().enumerate() {
            let line_len = line.chars().count();
            if current_offset + line_len >= char_offset {
                let col = char_offset - current_offset;
                return (line_num as u32, col as u32);
            }
            current_offset += line_len + 1; // +1 for newline
        }

        (0, 0)
    }

    /// Map part of speech to semantic token type
    fn pos_to_token_type(&self, pos: &str) -> u32 {
        match pos {
            "名詞" => 0,   // KEYWORD
            "動詞" => 1,   // FUNCTION
            "形容詞" => 2, // PROPERTY
            "副詞" => 3,   // VARIABLE
            "助詞" => 4,   // OPERATOR
            "助動詞" => 5, // STRING
            "接続詞" => 6, // NUMBER
            _ => 7,        // COMMENT (その他)
        }
    }
}
