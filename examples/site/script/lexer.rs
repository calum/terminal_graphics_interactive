//! Turns the text of a script into tokens, remembering where each one began so
//! errors can point at the right line and column.

use std::fmt;

/// A place in the source, counted from 1 like an editor shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pos {
    pub line: u32,
    pub column: u32,
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Ident(String),
    /// A number with a fraction, an exponent or an `f32` suffix.
    Float(f32),
    /// A whole number with no fraction.
    Int(i64),
    /// An operator or bracket.
    Punct(&'static str),
    /// Text or a character, which scripts do not use; the parser reports it where it appears.
    Text,
    Eof,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Ident(name) => write!(f, "`{name}`"),
            Token::Float(value) => write!(f, "`{value}`"),
            Token::Int(value) => write!(f, "`{value}`"),
            Token::Punct(text) => write!(f, "`{text}`"),
            Token::Text => write!(f, "text"),
            Token::Eof => write!(f, "the end of the program"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub pos: Pos,
    pub message: String,
}

/// Longest first, so `..` is not read as two dots and `<=` not as `<` then `=`.
const PUNCTUATION: &[&str] = &[
    "::", "->", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "..=", "..", "+", "-",
    "*", "/", "%", "(", ")", "[", "]", "{", "}", ",", ";", ":", ".", "|", "&", "!", "<", ">", "=",
    "?",
];

/// The tokens of `source`, each with where it starts, ending with `Eof`.
pub fn lex(source: &str) -> Result<Vec<(Token, Pos)>, Error> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let (mut i, mut line, mut column) = (0usize, 1u32, 1u32);

    // moves past one character, keeping the line and column right
    macro_rules! advance {
        () => {{
            if chars[i] == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
            i += 1;
        }};
    }

    while i < chars.len() {
        let c = chars[i];
        let pos = Pos { line, column };

        if c.is_whitespace() {
            advance!();
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                advance!();
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            advance!();
            advance!();
            loop {
                if i >= chars.len() {
                    return Err(Error {
                        pos,
                        message: "this comment is never closed".into(),
                    });
                }
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    advance!();
                    advance!();
                    break;
                }
                advance!();
            }
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
                advance!();
            }
            let mut is_float = false;
            // a fraction: `1.5`, or `1.` when no name or second dot follows
            if chars.get(i) == Some(&'.') {
                let next = chars.get(i + 1).copied();
                let fraction = next.is_some_and(|n| n.is_ascii_digit());
                let bare = !matches!(next, Some(n) if n == '.' || n == '_' || n.is_alphabetic());
                if fraction || bare {
                    is_float = true;
                    advance!();
                    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
                        advance!();
                    }
                }
            }
            // an exponent: `1e-3`
            if matches!(chars.get(i), Some('e') | Some('E')) {
                let sign = matches!(chars.get(i + 1), Some('+') | Some('-'));
                let digit = chars
                    .get(i + 1 + sign as usize)
                    .is_some_and(|d| d.is_ascii_digit());
                if digit {
                    is_float = true;
                    advance!();
                    if sign {
                        advance!();
                    }
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        advance!();
                    }
                }
            }
            let number: String = chars[start..i].iter().filter(|c| **c != '_').collect();

            // a type suffix: `2.0f32`, `3_f32`, `4i32`
            let suffix_start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                advance!();
            }
            let suffix: String = chars[suffix_start..i]
                .iter()
                .filter(|c| **c != '_')
                .collect();
            match suffix.as_str() {
                "" => {}
                "f32" => is_float = true,
                "f64" | "i32" | "i64" | "u32" | "u64" | "usize" | "isize" | "u8" | "i8" | "u16"
                | "i16" => {
                    return Err(Error {
                        pos,
                        message: format!(
                            "the number type `{suffix}` is not supported here; use `f32` for \
                             decimals and plain whole numbers for counts"
                        ),
                    });
                }
                _ => {
                    return Err(Error {
                        pos,
                        message: format!("`{suffix}` is not a valid number suffix"),
                    });
                }
            }

            let token = if is_float {
                number.parse::<f32>().map(Token::Float).map_err(|_| Error {
                    pos,
                    message: format!("`{number}` is not a valid number"),
                })?
            } else {
                number.parse::<i64>().map(Token::Int).map_err(|_| Error {
                    pos,
                    message: format!("`{number}` is too big"),
                })?
            };
            tokens.push((token, pos));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                advance!();
            }
            tokens.push((Token::Ident(chars[start..i].iter().collect()), pos));
        } else if let Some(text) = PUNCTUATION.iter().find(|text| {
            text.chars()
                .enumerate()
                .all(|(k, expected)| chars.get(i + k) == Some(&expected))
        }) {
            for _ in 0..text.len() {
                advance!();
            }
            tokens.push((Token::Punct(text), pos));
        } else if c == '"' {
            // text: skip to the closing quote, so the parser can say where it was used
            advance!();
            loop {
                if i >= chars.len() {
                    return Err(Error {
                        pos,
                        message: "this text is never closed".into(),
                    });
                }
                if chars[i] == '\\' && i + 1 < chars.len() {
                    advance!();
                } else if chars[i] == '"' {
                    advance!();
                    break;
                }
                advance!();
            }
            tokens.push((Token::Text, pos));
        } else if c == '\''
            && (chars.get(i + 2) == Some(&'\'')
                || (chars.get(i + 1) == Some(&'\\') && chars.get(i + 3) == Some(&'\'')))
        {
            // a character like 'X' or '\n'
            let length = if chars.get(i + 1) == Some(&'\\') {
                4
            } else {
                3
            };
            for _ in 0..length {
                advance!();
            }
            tokens.push((Token::Text, pos));
        } else {
            return Err(Error {
                pos,
                message: format!("unexpected character `{c}`"),
            });
        }
    }
    tokens.push((Token::Eof, Pos { line, column }));
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<Token> {
        lex(source).unwrap().into_iter().map(|(t, _)| t).collect()
    }

    fn ident(name: &str) -> Token {
        Token::Ident(name.to_string())
    }

    #[test]
    fn names_numbers_and_punctuation() {
        assert_eq!(
            kinds("let x = 1.5 + y;"),
            vec![
                ident("let"),
                ident("x"),
                Token::Punct("="),
                Token::Float(1.5),
                Token::Punct("+"),
                ident("y"),
                Token::Punct(";"),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn whole_numbers_are_ints_and_anything_with_a_fraction_is_a_float() {
        assert_eq!(kinds("7")[0], Token::Int(7));
        assert_eq!(kinds("7.0")[0], Token::Float(7.0));
        assert_eq!(kinds("7.")[0], Token::Float(7.0));
        assert_eq!(kinds("1e3")[0], Token::Float(1000.0));
        assert_eq!(kinds("2.5e-1")[0], Token::Float(0.25));
        assert_eq!(kinds("1_000")[0], Token::Int(1000));
    }

    #[test]
    fn a_suffix_makes_a_float_and_other_suffixes_are_refused() {
        assert_eq!(kinds("3f32")[0], Token::Float(3.0));
        assert_eq!(kinds("3_f32")[0], Token::Float(3.0));
        assert_eq!(kinds("0.5f32")[0], Token::Float(0.5));
        assert!(lex("3u8").unwrap_err().message.contains("not supported"));
        assert!(lex("3xyz")
            .unwrap_err()
            .message
            .contains("not a valid number suffix"));
    }

    #[test]
    fn a_range_is_not_swallowed_into_a_number() {
        assert_eq!(
            kinds("0..5"),
            vec![Token::Int(0), Token::Punct(".."), Token::Int(5), Token::Eof]
        );
    }

    #[test]
    fn a_method_on_a_decimal_is_a_dot_then_a_name() {
        assert_eq!(
            kinds("1.5.sin()"),
            vec![
                Token::Float(1.5),
                Token::Punct("."),
                ident("sin"),
                Token::Punct("("),
                Token::Punct(")"),
                Token::Eof
            ]
        );
    }

    #[test]
    fn the_longest_operator_wins() {
        assert_eq!(kinds("<=")[0], Token::Punct("<="));
        assert_eq!(kinds("::")[0], Token::Punct("::"));
        assert_eq!(kinds("->")[0], Token::Punct("->"));
        assert_eq!(kinds("+=")[0], Token::Punct("+="));
        assert_eq!(kinds("0..=5")[1], Token::Punct("..="));
        assert_eq!(kinds("&&")[0], Token::Punct("&&"));
    }

    #[test]
    fn comments_are_skipped_and_do_not_move_later_positions() {
        let tokens = lex("a // note\n/* x\ny */ b").unwrap();
        assert_eq!(tokens[1].0, ident("b"));
        assert_eq!(tokens[1].1, Pos { line: 3, column: 6 });
    }

    #[test]
    fn positions_count_lines_and_columns_from_one() {
        let tokens = lex("ab\n  cd").unwrap();
        assert_eq!(tokens[0].1, Pos { line: 1, column: 1 });
        assert_eq!(tokens[1].1, Pos { line: 2, column: 3 });
    }

    #[test]
    fn bad_input_gives_an_error_with_a_position_not_a_panic() {
        let error = lex("let x = 5 @ 2;").unwrap_err();
        assert_eq!(
            error.pos,
            Pos {
                line: 1,
                column: 11
            }
        );
        assert!(error.message.contains('@'));
        assert!(lex("/* never closed")
            .unwrap_err()
            .message
            .contains("never closed"));
        assert!(lex("99999999999999999999")
            .unwrap_err()
            .message
            .contains("too big"));
    }

    #[test]
    fn text_and_characters_become_one_token_so_the_parser_can_complain_in_order() {
        assert_eq!(kinds("\"hi there\"")[0], Token::Text);
        assert_eq!(kinds("'X'")[0], Token::Text);
        assert_eq!(kinds("'\\n'")[0], Token::Text);
        assert_eq!(
            kinds("\"a \\\" b\" 1")[1],
            Token::Int(1),
            "an escaped quote"
        );
        let tokens = lex("a \"x\" b").unwrap();
        assert_eq!(tokens[2].1, Pos { line: 1, column: 7 });
        assert!(lex("\"never closed")
            .unwrap_err()
            .message
            .contains("never closed"));
    }

    #[test]
    fn unicode_does_not_confuse_the_columns() {
        let tokens = lex("é x").unwrap();
        assert_eq!(tokens[1].1, Pos { line: 1, column: 3 });
    }

    #[test]
    fn nothing_panics_on_odd_inputs() {
        for source in [
            "", " ", "\n\n", "1.", ".", "..", "1e", "1e+", "/", "//", "/*", "*/", "0..", "_",
        ] {
            let _ = lex(source);
        }
    }
}
