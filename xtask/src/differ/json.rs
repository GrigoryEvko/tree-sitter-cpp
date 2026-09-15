//! A reader of JSON tokens with an explicit stack and no tree in memory.
//!
//! A Clang AST dump can have a depth of thousands of levels and a size of hundreds of megabytes.
//! This reader keeps one token at a time, and a limit stops it after a maximum number of bytes.

use std::fmt;
use std::io::{self, BufRead};

/// A token of a JSON document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    /// `{`
    BeginObject,
    /// `}`
    EndObject,
    /// `[`
    BeginArray,
    /// `]`
    EndArray,
    /// A key of an object. [`Lexer::text`] gives its text.
    Key,
    /// A string value. [`Lexer::text`] gives its text.
    Str,
    /// A number. The value is `None` if the number is not an integer from 0 to `u64::MAX`.
    Number(Option<u64>),
    /// `true` or `false`.
    Bool(bool),
    /// `null`.
    Null,
    /// The end of the input after the last value.
    End,
}

/// The cause of a read that did not complete.
#[derive(Debug)]
pub enum JsonError {
    /// The input has more bytes than the limit.
    TooLarge,
    /// The input is not valid JSON.
    Syntax(String),
    /// The input stream gave an error.
    Io(io::Error),
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonError::TooLarge => write!(f, "the JSON document is larger than the limit"),
            JsonError::Syntax(message) => write!(f, "the JSON document is not valid: {message}"),
            JsonError::Io(error) => write!(f, "cannot read the JSON document: {error}"),
        }
    }
}

impl From<io::Error> for JsonError {
    fn from(error: io::Error) -> Self {
        JsonError::Io(error)
    }
}

/// The kind of an open container.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Container {
    Object,
    Array,
}

/// A reader that gives the tokens of a JSON document in document order.
pub struct Lexer<R> {
    input: R,
    text: Vec<u8>,
    open: Vec<Container>,
    /// True when the next string in the innermost object is a key.
    expect_key: bool,
    read: u64,
    limit: u64,
}

impl<R: BufRead> Lexer<R> {
    /// A reader of `input` that stops with [`JsonError::TooLarge`] after `limit` bytes.
    pub fn new(input: R, limit: u64) -> Self {
        Lexer {
            input,
            text: Vec::new(),
            open: Vec::new(),
            expect_key: false,
            read: 0,
            limit,
        }
    }

    /// The number of bytes that the reader consumed.
    pub fn bytes(&self) -> u64 {
        self.read
    }

    /// The text of the last key or string. Text that is not valid UTF-8 gives an empty string.
    pub fn text(&self) -> &str {
        std::str::from_utf8(&self.text).unwrap_or("")
    }

    /// Consume `count` bytes of the input buffer, and count them against the limit.
    fn consume(&mut self, count: usize) -> Result<(), JsonError> {
        self.input.consume(count);
        self.read += count as u64;
        if self.read > self.limit {
            return Err(JsonError::TooLarge);
        }
        Ok(())
    }

    /// The next byte, or `None` at the end of the input. The byte stays in the input.
    fn peek(&mut self) -> Result<Option<u8>, JsonError> {
        Ok(self.input.fill_buf()?.first().copied())
    }

    /// Consume the next byte. The end of the input is a syntax error.
    fn byte(&mut self) -> Result<u8, JsonError> {
        let byte = self.peek()?.ok_or_else(|| self.syntax("the input stops in a value"))?;
        self.consume(1)?;
        Ok(byte)
    }

    /// A syntax error with the byte position of the reader.
    fn syntax(&self, message: &str) -> JsonError {
        JsonError::Syntax(format!("{message} at byte {}", self.read))
    }

    /// Read the next token. O(n) in the length of the token and of the white space before it.
    pub fn next_token(&mut self) -> Result<Token, JsonError> {
        loop {
            let buffer = self.input.fill_buf()?;
            let blank = buffer
                .iter()
                .position(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b':'))
                .unwrap_or(buffer.len());
            if blank > 0 {
                self.consume(blank)?;
                continue;
            }
            let Some(byte) = self.peek()? else {
                return if self.open.is_empty() {
                    Ok(Token::End)
                } else {
                    Err(self.syntax("the input stops in a container"))
                };
            };
            match byte {
                b',' => {
                    self.expect_key = self.open.last() == Some(&Container::Object);
                    self.consume(1)?;
                }
                b'{' => {
                    self.consume(1)?;
                    self.open.push(Container::Object);
                    self.expect_key = true;
                    return Ok(Token::BeginObject);
                }
                b'[' => {
                    self.consume(1)?;
                    self.open.push(Container::Array);
                    self.expect_key = false;
                    return Ok(Token::BeginArray);
                }
                b'}' | b']' => {
                    self.consume(1)?;
                    if self.open.pop().is_none() {
                        return Err(self.syntax("a container closes, but no container is open"));
                    }
                    self.expect_key = false;
                    return Ok(if byte == b'}' {
                        Token::EndObject
                    } else {
                        Token::EndArray
                    });
                }
                b'"' => {
                    self.consume(1)?;
                    self.string()?;
                    let key = self.expect_key && self.open.last() == Some(&Container::Object);
                    self.expect_key = false;
                    return Ok(if key { Token::Key } else { Token::Str });
                }
                b't' | b'f' | b'n' => return self.literal(),
                b'-' | b'0'..=b'9' => return self.number(),
                _ => return Err(self.syntax(&format!("the byte 0x{byte:02x} cannot start a token"))),
            }
        }
    }

    /// Read the rest of a string after its opening quote into the text buffer.
    fn string(&mut self) -> Result<(), JsonError> {
        self.text.clear();
        loop {
            let buffer = self.input.fill_buf()?;
            if buffer.is_empty() {
                return Err(self.syntax("the input stops in a string"));
            }
            match buffer.iter().position(|&b| b == b'"' || b == b'\\') {
                None => {
                    let count = buffer.len();
                    self.text.extend_from_slice(buffer);
                    self.consume(count)?;
                }
                Some(at) => {
                    let quote = buffer[at] == b'"';
                    self.text.extend_from_slice(&buffer[..at]);
                    self.consume(at + 1)?;
                    if quote {
                        return Ok(());
                    }
                    self.escape()?;
                }
            }
        }
    }

    /// Read an escape sequence after its backslash, and add its character to the text buffer.
    fn escape(&mut self) -> Result<(), JsonError> {
        let simple = match self.byte()? {
            b'"' => b'"',
            b'\\' => b'\\',
            b'/' => b'/',
            b'b' => 0x08,
            b'f' => 0x0c,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'u' => {
                let mut code = self.hex4()?;
                if (0xd800..0xdc00).contains(&code) {
                    if self.byte()? != b'\\' || self.byte()? != b'u' {
                        return Err(self.syntax("a high surrogate has no low surrogate"));
                    }
                    let low = self.hex4()?;
                    code = 0x10000 + ((code - 0xd800) << 10) + (low.wrapping_sub(0xdc00) & 0x3ff);
                }
                let character = char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER);
                let mut utf8 = [0; 4];
                self.text.extend_from_slice(character.encode_utf8(&mut utf8).as_bytes());
                return Ok(());
            }
            _ => return Err(self.syntax("the escape sequence is not valid")),
        };
        self.text.push(simple);
        Ok(())
    }

    /// Read the four hexadecimal digits of a `\u` escape.
    fn hex4(&mut self) -> Result<u32, JsonError> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = char::from(self.byte()?)
                .to_digit(16)
                .ok_or_else(|| self.syntax("a \\u escape has a byte that is not a hexadecimal digit"))?;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    /// Read `true`, `false`, or `null`.
    fn literal(&mut self) -> Result<Token, JsonError> {
        self.text.clear();
        while let Some(byte) = self.peek()? {
            if !byte.is_ascii_alphabetic() {
                break;
            }
            self.text.push(byte);
            self.consume(1)?;
        }
        self.expect_key = false;
        match self.text.as_slice() {
            b"true" => Ok(Token::Bool(true)),
            b"false" => Ok(Token::Bool(false)),
            b"null" => Ok(Token::Null),
            _ => Err(self.syntax("a literal is not true, false, or null")),
        }
    }

    /// Read a number. Only an integer from 0 to `u64::MAX` gives a value.
    fn number(&mut self) -> Result<Token, JsonError> {
        let mut value = Some(0u64);
        let mut digits = false;
        while let Some(byte) = self.peek()? {
            match byte {
                b'0'..=b'9' => {
                    value = value
                        .and_then(|v| v.checked_mul(10))
                        .and_then(|v| v.checked_add(u64::from(byte - b'0')));
                    digits = true;
                }
                b'-' | b'+' | b'.' | b'e' | b'E' => value = None,
                _ => break,
            }
            self.consume(1)?;
        }
        self.expect_key = false;
        if !digits {
            return Err(self.syntax("a number has no digits"));
        }
        Ok(Token::Number(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All the tokens of a document, with the text of each key and string.
    fn tokens(text: &str, limit: u64) -> Result<Vec<(Token, String)>, JsonError> {
        let mut lexer = Lexer::new(text.as_bytes(), limit);
        let mut out = Vec::new();
        loop {
            let token = lexer.next_token()?;
            let text = match token {
                Token::Key | Token::Str => lexer.text().to_owned(),
                _ => String::new(),
            };
            out.push((token, text));
            if token == Token::End {
                return Ok(out);
            }
        }
    }

    #[test]
    fn keys_values_and_escapes_come_in_document_order() {
        let found =
            tokens(r#"{"a": [1, -2, "x\"é😀"], "b": {"c": true, "d": null}}"#, 1000).expect("the document is valid");
        let expected = vec![
            (Token::BeginObject, ""),
            (Token::Key, "a"),
            (Token::BeginArray, ""),
            (Token::Number(Some(1)), ""),
            (Token::Number(None), ""),
            (Token::Str, "x\"é😀"),
            (Token::EndArray, ""),
            (Token::Key, "b"),
            (Token::BeginObject, ""),
            (Token::Key, "c"),
            (Token::Bool(true), ""),
            (Token::Key, "d"),
            (Token::Null, ""),
            (Token::EndObject, ""),
            (Token::EndObject, ""),
            (Token::End, ""),
        ];
        let expected: Vec<(Token, String)> = expected.into_iter().map(|(t, s)| (t, s.to_owned())).collect();
        assert_eq!(found, expected);
    }

    #[test]
    fn a_string_value_in_an_object_is_not_a_key() {
        let found = tokens(r#"{"k": "v", "w": ["s"]}"#, 100).expect("the document is valid");
        let kinds: Vec<Token> = found.iter().map(|(t, _)| *t).collect();
        assert_eq!(
            kinds,
            [
                Token::BeginObject,
                Token::Key,
                Token::Str,
                Token::Key,
                Token::BeginArray,
                Token::Str,
                Token::EndArray,
                Token::EndObject,
                Token::End
            ]
        );
    }

    #[test]
    fn the_limit_stops_a_large_document() {
        assert!(matches!(tokens(r#"{"a": "0123456789"}"#, 8), Err(JsonError::TooLarge)));
    }
}
