use eframe::egui::{self, Color32, FontId, TextFormat};

const COL_PLAIN: Color32 = Color32::from_rgb(205, 214, 244);
const COL_KEYWORD: Color32 = Color32::from_rgb(203, 166, 247);
const COL_TYPE: Color32 = Color32::from_rgb(137, 180, 250);
const COL_STRING: Color32 = Color32::from_rgb(166, 227, 161);
const COL_NUMBER: Color32 = Color32::from_rgb(250, 179, 135);
const COL_COMMENT: Color32 = Color32::from_rgb(108, 112, 134);
const COL_PUNCT: Color32 = Color32::from_rgb(148, 226, 213);

/// Ultra-lightweight syntax highlighter with zero persistent AST memory overhead.
pub fn highlight_code(text: &str, ext: &str, font_size: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let font_id = FontId::monospace(font_size);
    let keywords = keywords_for_ext(ext);
    let hash_comment = matches!(ext, "py" | "sh" | "bash" | "zsh" | "toml" | "yaml" | "yml");

    for (line_idx, line) in text.split('\n').enumerate() {
        if line_idx > 0 {
            job.append("\n", 0.0, fmt(font_id.clone(), COL_PLAIN));
        }
        highlight_line(line, keywords, hash_comment, &font_id, &mut job);
    }

    job
}

fn highlight_line(
    line: &str,
    keywords: &[&str],
    hash_comment: bool,
    font_id: &FontId,
    job: &mut egui::text::LayoutJob,
) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Line comments: // or #
        if (i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/')
            || (hash_comment && bytes[i] == b'#')
        {
            job.append(&line[i..], 0.0, fmt(font_id.clone(), COL_COMMENT));
            return;
        }

        // String literals: "..." or '...'
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            job.append(&line[start..i], 0.0, fmt(font_id.clone(), COL_STRING));
            continue;
        }

        // Numbers
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_') {
                i += 1;
            }
            job.append(&line[start..i], 0.0, fmt(font_id.clone(), COL_NUMBER));
            continue;
        }

        // Identifiers / Keywords / Types
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &line[start..i];
            let color = if keywords.contains(&word) {
                COL_KEYWORD
            } else if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                COL_TYPE
            } else {
                COL_PLAIN
            };
            job.append(word, 0.0, fmt(font_id.clone(), color));
            continue;
        }

        // Punctuation / Operators
        let ch = line[i..].chars().next().unwrap();
        let ch_len = ch.len_utf8();
        let color = if matches!(ch, '{' | '}' | '(' | ')' | '[' | ']' | ':' | ';' | '=' | '<' | '>') {
            COL_PUNCT
        } else {
            COL_PLAIN
        };
        job.append(&line[i..i + ch_len], 0.0, fmt(font_id.clone(), color));
        i += ch_len;
    }
}

fn fmt(font_id: FontId, color: Color32) -> TextFormat {
    TextFormat {
        font_id,
        color,
        ..Default::default()
    }
}

fn keywords_for_ext(ext: &str) -> &'static [&'static str] {
    match ext {
        "rs" => &[
            "fn", "let", "mut", "pub", "struct", "enum", "impl", "trait", "for", "while", "loop",
            "if", "else", "match", "return", "use", "mod", "const", "static", "type", "where",
            "async", "await", "move", "ref", "in", "as", "break", "continue", "self", "super",
            "crate", "true", "false", "unsafe",
        ],
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => &[
            "function", "const", "let", "var", "return", "if", "else", "for", "while", "switch",
            "case", "break", "continue", "import", "export", "from", "default", "class", "extends",
            "new", "this", "async", "await", "try", "catch", "finally", "throw", "interface",
            "type", "enum", "public", "private", "true", "false", "null", "undefined",
        ],
        "py" => &[
            "def", "class", "return", "if", "elif", "else", "for", "while", "in", "import", "from",
            "as", "pass", "break", "continue", "try", "except", "finally", "with", "lambda",
            "yield", "async", "await", "True", "False", "None", "and", "or", "not", "is",
        ],
        "go" => &[
            "func", "package", "import", "var", "const", "type", "struct", "interface", "return",
            "if", "else", "for", "range", "switch", "case", "default", "go", "defer", "chan",
            "select", "map", "true", "false", "nil",
        ],
        "c" | "h" | "cpp" | "hpp" | "cc" => &[
            "int", "void", "char", "float", "double", "bool", "const", "static", "struct", "enum",
            "class", "public", "private", "return", "if", "else", "for", "while", "switch", "case",
            "break", "continue", "include", "define", "sizeof", "auto", "nullptr", "true", "false",
        ],
        "json" => &["true", "false", "null"],
        _ => &["if", "else", "for", "while", "return", "true", "false"],
    }
}
