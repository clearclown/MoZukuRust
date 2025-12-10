use std::sync::Arc;

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::analyzer::{MorphologicalAnalyzer, TokenInfo};

/// Grammar checker for Japanese text
pub struct GrammarChecker {
    analyzer: Arc<MorphologicalAnalyzer>,
}

impl GrammarChecker {
    pub fn new(analyzer: Arc<MorphologicalAnalyzer>) -> Self {
        Self { analyzer }
    }

    /// Check text and return diagnostics
    pub fn check(&self, text: &str) -> Vec<Diagnostic> {
        let tokens = self.analyzer.tokenize(text);
        let lines: Vec<&str> = text.lines().collect();

        let mut diagnostics = Vec::new();

        // Run all checks
        diagnostics.extend(self.check_ra_nuki(&tokens, &lines));
        diagnostics.extend(self.check_i_nuki(&tokens, &lines));
        diagnostics.extend(self.check_double_particle(&tokens, &lines));
        diagnostics.extend(self.check_redundant_na(&tokens, &lines));

        // Phase 3: Additional checks
        diagnostics.extend(self.check_double_honorific(&tokens, &lines));
        diagnostics.extend(self.check_redundant_expression(&tokens, &lines));
        diagnostics.extend(self.check_consecutive_sentence_endings(text));
        diagnostics.extend(self.check_tari_parallel(&tokens, &lines));
        diagnostics.extend(self.check_consecutive_no(&tokens, &lines));

        diagnostics
    }

    /// Check for ら抜き言葉 (ra-nuki kotoba)
    /// Example: 食べれる → 食べられる
    fn check_ra_nuki(&self, tokens: &[TokenInfo], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (i, token) in tokens.iter().enumerate() {
            // Check for 一段動詞 (ichidan verbs) + れる pattern
            if token.pos == "動詞"
                && token.conjugation_type.contains("一段")
                && token.surface.ends_with("れる")
            {
                // Check if this might be ra-nuki
                let base = &token.base_form;
                if base.ends_with("れる") && !base.ends_with("られる") {
                    // Likely ra-nuki
                    let range = self.token_to_range(token, lines);
                    let correct_form = token.surface.replacen("れる", "られる", 1);

                    diagnostics.push(Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(tower_lsp::lsp_types::NumberOrString::String(
                            "ra-nuki".to_string(),
                        )),
                        source: Some("mozuku".to_string()),
                        message: format!(
                            "ら抜き言葉の可能性があります。「{}」→「{}」",
                            token.surface, correct_form
                        ),
                        ..Default::default()
                    });
                }
            }

            // Also check for pattern: 動詞連用形 + れる
            if i > 0 && token.surface == "れる" && token.pos == "動詞" {
                let prev = &tokens[i - 1];
                if prev.pos == "動詞"
                    && prev.conjugation_type.contains("一段")
                    && prev.conjugation_form.contains("連用形")
                {
                    let range = self.tokens_to_range(&[prev, token], lines);
                    let combined = format!("{}{}", prev.surface, token.surface);
                    let correct = format!("{}られる", prev.surface);

                    diagnostics.push(Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(tower_lsp::lsp_types::NumberOrString::String(
                            "ra-nuki".to_string(),
                        )),
                        source: Some("mozuku".to_string()),
                        message: format!(
                            "ら抜き言葉の可能性があります。「{}」→「{}」",
                            combined, correct
                        ),
                        ..Default::default()
                    });
                }
            }
        }

        diagnostics
    }

    /// Check for い抜き言葉 (i-nuki kotoba)
    /// Example: している → してる
    fn check_i_nuki(&self, tokens: &[TokenInfo], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (i, token) in tokens.iter().enumerate() {
            // Check for てる pattern (should be ている)
            if i > 0 && token.surface == "てる" && token.pos == "助動詞" {
                let prev = &tokens[i - 1];
                if prev.pos == "動詞" {
                    let range = self.token_to_range(token, lines);
                    diagnostics.push(Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::HINT),
                        code: Some(tower_lsp::lsp_types::NumberOrString::String(
                            "i-nuki".to_string(),
                        )),
                        source: Some("mozuku".to_string()),
                        message: "い抜き言葉です。「てる」→「ている」（口語では許容）".to_string(),
                        ..Default::default()
                    });
                }
            }

            // Check for でる pattern (should be でいる)
            if i > 0 && token.surface == "でる" && token.pos == "助動詞" {
                let prev = &tokens[i - 1];
                if prev.pos == "動詞" {
                    let range = self.token_to_range(token, lines);
                    diagnostics.push(Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::HINT),
                        code: Some(tower_lsp::lsp_types::NumberOrString::String(
                            "i-nuki".to_string(),
                        )),
                        source: Some("mozuku".to_string()),
                        message: "い抜き言葉です。「でる」→「でいる」（口語では許容）".to_string(),
                        ..Default::default()
                    });
                }
            }
        }

        diagnostics
    }

    /// Check for double particles (二重助詞)
    /// Example: がが, をを, にに
    fn check_double_particle(&self, tokens: &[TokenInfo], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Particles to check for duplication
        let target_particles = ["が", "を", "に", "へ", "で", "と", "から", "まで", "より"];

        for i in 0..tokens.len().saturating_sub(1) {
            let current = &tokens[i];
            let next = &tokens[i + 1];

            // Check if both are the same particle
            if current.pos == "助詞"
                && next.pos == "助詞"
                && current.surface == next.surface
                && target_particles.contains(&current.surface.as_str())
            {
                let range = self.tokens_to_range(&[current, next], lines);
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(tower_lsp::lsp_types::NumberOrString::String(
                        "double-particle".to_string(),
                    )),
                    source: Some("mozuku".to_string()),
                    message: format!(
                        "助詞「{}」が重複しています。",
                        current.surface
                    ),
                    ..Default::default()
                });
            }
        }

        diagnostics
    }

    /// Check for redundant な with na-adjectives
    /// Example: 静かなな → 静かな
    fn check_redundant_na(&self, tokens: &[TokenInfo], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for i in 0..tokens.len().saturating_sub(1) {
            let current = &tokens[i];
            let next = &tokens[i + 1];

            // Check for な + な pattern
            if current.surface == "な"
                && next.surface == "な"
                && current.pos == "助動詞"
                && next.pos == "助動詞"
            {
                let range = self.tokens_to_range(&[current, next], lines);
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(tower_lsp::lsp_types::NumberOrString::String(
                        "redundant-na".to_string(),
                    )),
                    source: Some("mozuku".to_string()),
                    message: "「な」が重複しています。".to_string(),
                    ..Default::default()
                });
            }
        }

        diagnostics
    }

    /// Check for double honorific (二重敬語)
    /// Example: おっしゃられる → おっしゃる, ご覧になられる → ご覧になる
    fn check_double_honorific(&self, tokens: &[TokenInfo], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Common honorific verb stems that should not be followed by れる/られる
        // Lindera may split "おっしゃられ" into "おっしゃら" + "れ"
        let honorific_stems = [
            ("おっしゃ", "おっしゃる"),     // おっしゃら + れ
            ("いらっしゃ", "いらっしゃる"), // いらっしゃら + れ
            ("なさ", "なさる"),             // なさら + れ
            ("くださ", "くださる"),         // くださら + れ
            ("召し上が", "召し上がる"),     // 召し上がら + れ
        ];

        // Check for stem + れ pattern (e.g., おっしゃら + れ)
        for i in 0..tokens.len().saturating_sub(1) {
            let current = &tokens[i];
            let next = &tokens[i + 1];

            // Check if current token is an honorific stem
            for (stem, correct) in &honorific_stems {
                if current.surface.starts_with(stem)
                    && current.pos == "動詞"
                    && (next.surface == "れ" || next.surface == "られ")
                    && next.pos == "動詞"
                {
                    let range = self.tokens_to_range(&[current, next], lines);
                    diagnostics.push(Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(tower_lsp::lsp_types::NumberOrString::String(
                            "double-honorific".to_string(),
                        )),
                        source: Some("mozuku".to_string()),
                        message: format!(
                            "二重敬語の可能性があります。「{}{}」→「{}」",
                            current.surface, next.surface, correct
                        ),
                        ..Default::default()
                    });
                    break;
                }
            }
        }

        // Check for ご〜になられる pattern
        // Lindera splits: ご覧 + に + なら + れ
        for i in 0..tokens.len().saturating_sub(3) {
            let t0 = &tokens[i];
            let t1 = &tokens[i + 1];
            let t2 = &tokens[i + 2];
            let t3 = &tokens[i + 3];

            // Pattern: ご〜 + に + なら + れ
            if t0.surface.starts_with("ご")
                && t1.surface == "に"
                && (t2.surface == "なら" || t2.surface == "なり")
                && (t3.surface == "れ" || t3.surface == "られ")
            {
                let range = self.tokens_to_range(&[t0, t1, t2, t3], lines);
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(tower_lsp::lsp_types::NumberOrString::String(
                        "double-honorific".to_string(),
                    )),
                    source: Some("mozuku".to_string()),
                    message: format!(
                        "二重敬語の可能性があります。「{}{}{}{}」→「{}になる」",
                        t0.surface, t1.surface, t2.surface, t3.surface, t0.surface
                    ),
                    ..Default::default()
                });
            }
        }

        diagnostics
    }

    /// Check for redundant expressions (冗長表現)
    /// Example: することができる → できる, ことが可能 → できる
    fn check_redundant_expression(&self, tokens: &[TokenInfo], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Look for patterns like: Verb + こと + が + できる/可能
        for i in 0..tokens.len().saturating_sub(3) {
            let t0 = &tokens[i];
            let t1 = &tokens[i + 1];
            let t2 = &tokens[i + 2];
            let t3 = if i + 3 < tokens.len() {
                Some(&tokens[i + 3])
            } else {
                None
            };

            // Pattern: こと + が + できる
            if t0.surface == "こと" && t1.surface == "が" {
                if t3.is_some() {
                    if t2.surface == "でき" || t2.base_form == "できる" {
                        let range = self.tokens_to_range(&[t0, t1, t2], lines);
                        diagnostics.push(Diagnostic {
                            range,
                            severity: Some(DiagnosticSeverity::HINT),
                            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                                "redundant-expression".to_string(),
                            )),
                            source: Some("mozuku".to_string()),
                            message: "冗長な表現です。「〜ことができる」→「〜できる」".to_string(),
                            ..Default::default()
                        });
                    } else if t2.surface == "可能" {
                        let range = self.tokens_to_range(&[t0, t1, t2], lines);
                        diagnostics.push(Diagnostic {
                            range,
                            severity: Some(DiagnosticSeverity::HINT),
                            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                                "redundant-expression".to_string(),
                            )),
                            source: Some("mozuku".to_string()),
                            message: "冗長な表現です。「〜ことが可能」→「〜できる」".to_string(),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        diagnostics
    }

    /// Check for consecutive same sentence endings (連続する同じ文末)
    /// Example: です。です。です。
    fn check_consecutive_sentence_endings(&self, text: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Split by sentence-ending punctuation and analyze
        let sentences: Vec<&str> = text.split('。').filter(|s| !s.is_empty()).collect();

        if sentences.len() < 3 {
            return diagnostics;
        }

        // Track consecutive endings
        let mut consecutive_count = 1;
        let mut last_ending = String::new();

        for (i, sentence) in sentences.iter().enumerate() {
            let trimmed = sentence.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Get the last few characters as the ending pattern
            let ending = if trimmed.ends_with("です") {
                "です"
            } else if trimmed.ends_with("ます") {
                "ます"
            } else if trimmed.ends_with("である") {
                "である"
            } else if trimmed.ends_with("だ") {
                "だ"
            } else {
                ""
            };

            if ending.is_empty() {
                consecutive_count = 1;
                last_ending = String::new();
                continue;
            }

            if ending == last_ending {
                consecutive_count += 1;
            } else {
                consecutive_count = 1;
                last_ending = ending.to_string();
            }

            // Report if 3 or more consecutive same endings
            if consecutive_count >= 3 {
                // Calculate approximate position
                let char_offset: usize = sentences[..=i]
                    .iter()
                    .map(|s| s.chars().count() + 1) // +1 for 。
                    .sum();

                let lines: Vec<&str> = text.lines().collect();
                let (line, col) = self.char_offset_to_position(&lines, char_offset.saturating_sub(3));

                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position { line, character: col },
                        end: Position { line, character: col + 2 },
                    },
                    severity: Some(DiagnosticSeverity::HINT),
                    code: Some(tower_lsp::lsp_types::NumberOrString::String(
                        "consecutive-endings".to_string(),
                    )),
                    source: Some("mozuku".to_string()),
                    message: format!(
                        "同じ文末「{}」が{}回連続しています。文体の変化を検討してください。",
                        last_ending, consecutive_count
                    ),
                    ..Default::default()
                });

                // Reset to avoid multiple warnings
                consecutive_count = 1;
            }
        }

        diagnostics
    }

    /// Check for incomplete たり parallel (たり〜たり の不完全な並列)
    /// Example: 歩いたり走る → 歩いたり走ったりする
    fn check_tari_parallel(&self, tokens: &[TokenInfo], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Find たり and check if there's a matching たり
        let tari_positions: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| t.surface == "たり")
            .map(|(i, _)| i)
            .collect();

        // If there's exactly one たり, it might be incomplete
        if tari_positions.len() == 1 {
            let tari_idx = tari_positions[0];
            let tari_token = &tokens[tari_idx];

            // Check if followed by verb without another たり
            let has_following_verb = tokens[tari_idx + 1..]
                .iter()
                .any(|t| t.pos == "動詞" && !t.surface.ends_with("たり"));

            let has_following_tari = tokens[tari_idx + 1..]
                .iter()
                .any(|t| t.surface == "たり" || t.surface.ends_with("たり"));

            if has_following_verb && !has_following_tari {
                let range = self.token_to_range(tari_token, lines);
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(tower_lsp::lsp_types::NumberOrString::String(
                        "incomplete-tari".to_string(),
                    )),
                    source: Some("mozuku".to_string()),
                    message: "「たり」を使う場合は「〜たり〜たりする」の形が適切です。".to_string(),
                    ..Default::default()
                });
            }
        }

        diagnostics
    }

    /// Check for consecutive の particles (「の」の連続使用)
    /// Pattern: 名詞の名詞の名詞の... (3つ以上の「の」は警告)
    fn check_consecutive_no(&self, tokens: &[TokenInfo], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Find sequences like: 名詞 + の + 名詞 + の + 名詞 + の + ...
        // Track の particles and their positions
        let mut no_positions: Vec<&TokenInfo> = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            let token = &tokens[i];

            // Start of potential sequence: 名詞 or already tracking
            if token.surface == "の" && token.pos == "助詞" {
                // Check if preceded by 名詞
                if i > 0 && tokens[i - 1].pos == "名詞" {
                    no_positions.push(token);
                } else {
                    // Reset if の is not preceded by 名詞
                    if no_positions.len() >= 3 {
                        self.report_consecutive_no(&no_positions, lines, &mut diagnostics);
                    }
                    no_positions.clear();
                }
            } else if token.pos != "名詞" && !no_positions.is_empty() {
                // Non-noun token (not の) breaks the sequence
                if no_positions.len() >= 3 {
                    self.report_consecutive_no(&no_positions, lines, &mut diagnostics);
                }
                no_positions.clear();
            }

            i += 1;
        }

        // Check remaining sequence
        if no_positions.len() >= 3 {
            self.report_consecutive_no(&no_positions, lines, &mut diagnostics);
        }

        diagnostics
    }

    /// Helper to report consecutive の warning
    fn report_consecutive_no(
        &self,
        no_positions: &[&TokenInfo],
        lines: &[&str],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let first = no_positions.first().unwrap();
        let last = no_positions.last().unwrap();

        let (start_line, start_col) = self.char_offset_to_position(lines, first.char_offset);
        let (end_line, end_col) =
            self.char_offset_to_position(lines, last.char_offset + last.char_length);

        diagnostics.push(Diagnostic {
            range: Range {
                start: Position {
                    line: start_line,
                    character: start_col,
                },
                end: Position {
                    line: end_line,
                    character: end_col,
                },
            },
            severity: Some(DiagnosticSeverity::HINT),
            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                "consecutive-no".to_string(),
            )),
            source: Some("mozuku".to_string()),
            message: format!(
                "「の」が{}回連続しています。読みやすさのため言い換えを検討してください。",
                no_positions.len()
            ),
            ..Default::default()
        });
    }

    /// Convert token position to LSP range
    fn token_to_range(&self, token: &TokenInfo, lines: &[&str]) -> Range {
        let (start_line, start_col) = self.char_offset_to_position(lines, token.char_offset);
        let (end_line, end_col) =
            self.char_offset_to_position(lines, token.char_offset + token.char_length);

        Range {
            start: Position {
                line: start_line,
                character: start_col,
            },
            end: Position {
                line: end_line,
                character: end_col,
            },
        }
    }

    /// Convert multiple tokens to a single range
    fn tokens_to_range(&self, tokens: &[&TokenInfo], lines: &[&str]) -> Range {
        let first = tokens.first().unwrap();
        let last = tokens.last().unwrap();

        let (start_line, start_col) = self.char_offset_to_position(lines, first.char_offset);
        let (end_line, end_col) =
            self.char_offset_to_position(lines, last.char_offset + last.char_length);

        Range {
            start: Position {
                line: start_line,
                character: start_col,
            },
            end: Position {
                line: end_line,
                character: end_col,
            },
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_checker() -> GrammarChecker {
        let analyzer = Arc::new(MorphologicalAnalyzer::new().unwrap());
        GrammarChecker::new(analyzer)
    }

    #[test]
    fn test_double_particle() {
        let checker = setup_checker();
        let text = "私がが行く";
        let diagnostics = checker.check(text);

        assert!(!diagnostics.is_empty());
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("助詞") && d.message.contains("重複")));
    }

    #[test]
    fn test_no_false_positive() {
        let checker = setup_checker();
        let text = "私は本を読む";
        let diagnostics = checker.check(text);

        // Should have no errors for correct text
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect();
        assert!(errors.is_empty());
    }

    // ==========================================
    // Phase 3: 追加文法ルールのテスト
    // ==========================================

    #[test]
    fn test_double_honorific_osshareru() {
        // おっしゃられる → おっしゃる（二重敬語）
        let checker = setup_checker();
        let text = "先生がおっしゃられました";
        let diagnostics = checker.check(text);

        assert!(
            diagnostics.iter().any(|d| d.message.contains("二重敬語")),
            "Should detect double honorific: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_double_honorific_goranninaru() {
        // ご覧になられる → ご覧になる（二重敬語）
        let checker = setup_checker();
        let text = "資料をご覧になられてください";
        let diagnostics = checker.check(text);

        assert!(
            diagnostics.iter().any(|d| d.message.contains("二重敬語")),
            "Should detect double honorific: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_redundant_suru_koto_ga_dekiru() {
        // することができる → できる
        let checker = setup_checker();
        let text = "私は泳ぐことができます";
        let diagnostics = checker.check(text);

        assert!(
            diagnostics.iter().any(|d| d.message.contains("冗長")),
            "Should detect redundant expression: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_redundant_koto_ga_kanou() {
        // ことが可能 → できる
        let checker = setup_checker();
        let text = "参加することが可能です";
        let diagnostics = checker.check(text);

        assert!(
            diagnostics.iter().any(|d| d.message.contains("冗長")),
            "Should detect redundant expression: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_consecutive_sentence_endings() {
        // 連続する同じ文末
        let checker = setup_checker();
        let text = "私は学生です。彼も学生です。彼女も学生です。";
        let diagnostics = checker.check(text);

        assert!(
            diagnostics.iter().any(|d| d.message.contains("文末") || d.message.contains("連続")),
            "Should detect consecutive same endings: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_tari_parallel_incomplete() {
        // たり〜たり の不完全な並列
        let checker = setup_checker();
        let text = "歩いたり走る";
        let diagnostics = checker.check(text);

        assert!(
            diagnostics.iter().any(|d| d.message.contains("たり")),
            "Should detect incomplete tari parallel: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_tari_parallel_correct() {
        // 正しい「たり〜たり」
        let checker = setup_checker();
        let text = "歩いたり走ったりする";
        let diagnostics = checker.check(text);

        // This should NOT trigger tari error
        let tari_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("たり"))
            .collect();
        assert!(
            tari_errors.is_empty(),
            "Should not flag correct tari usage: {:?}",
            tari_errors
        );
    }

    #[test]
    fn test_consecutive_no_particles() {
        // 「の」の連続使用
        let checker = setup_checker();
        let text = "私の友達の本の内容";
        let diagnostics = checker.check(text);

        assert!(
            diagnostics.iter().any(|d| d.message.contains("の") && d.message.contains("連続")),
            "Should detect consecutive no particles: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_two_no_particles_ok() {
        // 2つまでの「の」は許容
        let checker = setup_checker();
        let text = "私の本の内容";
        let diagnostics = checker.check(text);

        // Should not trigger for just 2 consecutive の
        let no_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.message.contains("の") && d.message.contains("連続"))
            .collect();
        assert!(
            no_errors.is_empty(),
            "Should allow 2 consecutive no: {:?}",
            no_errors
        );
    }
}
