use causm_core::symbol::{intern, Symbol};
use causm_core::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Ident(Symbol),
    Int(i64),
    Duration(u64),
    Float(String),
    Str(String),
    FStr(String),
    ByteStr(Vec<u8>),
    HexByteStr(Vec<u8>),
    Bool(bool),
    Null,

    // Keywords
    Let,
    Yield,
    Return,
    If,
    Else,
    Match,
    Loop,
    While,
    Break,
    Continue,
    Routine,
    Taking,
    Actor,
    Require,
    On,
    Send,
    To,
    Isolate,
    Enable,
    Lease,
    For,
    In,
    Step,
    Import,
    From,
    As,
    Type,
    Struct,
    Enum,
    Interface,
    Using,
    Reconcile,
    Speculate,
    Commit,
    Fallback,
    Collapse,
    Anchor,
    RewindTo,
    Entangle,
    AssertTime,
    Slice,
    Print,
    Tick,
    Max,
    Mut,
    Pub,
    Foreign,
    Macro,
    State,
    Policy,
    Select,
    Case,
    Timeout,
    Valid,
    Decayed,
    Pending,
    Consumed,
    Defer,
    ChanRecv,
    Syscall,
    Arena,
    Distinct,
    Split,
    Merge,
    Into,
    Auto,
    Debug,
    Log,
    OnDecay,
    Where,

    // Symbols & Punctuation
    At,
    Colon,
    DoubleColon,
    Semi,
    Comma,
    Dot,
    Question,
    Pipe,
    Pipeline,
    Arrow,
    FatArrow,
    Eq,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Plus,
    PlusEq,
    Minus,
    MinusEq,
    Star,
    StarStar,
    Slash,
    Percent,
    Amp,
    AmpAmp,
    PipePipe,
    Caret,
    Shl,
    Shr,
    NullCoalesce,
    Bang,
    Tilde,
    StarEq,
    SlashEq,
    PercentEq,
    ShlEq,
    ShrEq,
    AmpEq,
    PipeEq,
    CaretEq,
    DotDot,
    DotDotEq,
    Dollar,
    DoubleArrow,

    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    Eof,
}

impl TokenKind {
    pub fn as_ident_symbol(&self) -> Option<Symbol> {
        match self {
            TokenKind::Ident(sym) => Some(*sym),
            TokenKind::State => Some(intern("state")),
            TokenKind::Policy => Some(intern("policy")),
            TokenKind::Select => Some(intern("select")),
            TokenKind::Tick => Some(intern("tick")),
            TokenKind::Max => Some(intern("max")),
            TokenKind::Valid => Some(intern("Valid")),
            TokenKind::Decayed => Some(intern("Decayed")),
            TokenKind::Pending => Some(intern("Pending")),
            TokenKind::Consumed => Some(intern("Consumed")),
            TokenKind::Auto => Some(intern("auto")),
            TokenKind::Debug => Some(intern("debug")),
            TokenKind::Log => Some(intern("log")),
            TokenKind::Slice => Some(intern("slice")),
            TokenKind::Print => Some(intern("print")),
            TokenKind::Step => Some(intern("step")),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone)]
pub struct TokenStream<'a> {
    pub source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
}

impl<'a> TokenStream<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
        }
    }

    pub fn peek_token(&self) -> Token {
        let mut clone = self.clone();
        clone.next_token()
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(&(_, ch)) = self.chars.peek() {
            if ch.is_whitespace() {
                self.chars.next();
            } else if ch == '/' {
                let mut clone = self.chars.clone();
                clone.next();
                if let Some((_, '/')) = clone.peek() {
                    self.chars.next();
                    self.chars.next();
                    while let Some((_, ch2)) = self.chars.next() {
                        if ch2 == '\n' {
                            break;
                        }
                    }
                } else if let Some((_, '*')) = clone.peek() {
                    self.chars.next();
                    self.chars.next();
                    while let Some((_, ch2)) = self.chars.next() {
                        if ch2 == '*' {
                            if let Some(&(_, '/')) = self.chars.peek() {
                                self.chars.next();
                                break;
                            }
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();

        let (start, ch) = match self.chars.next() {
            Some(pair) => pair,
            None => {
                let len = self.source.len();
                return Token {
                    kind: TokenKind::Eof,
                    span: Span {
                        start: len,
                        end: len,
                    },
                };
            }
        };

        let kind = match ch {
            '@' => TokenKind::At,
            ':' => {
                if let Some(&(_, ':')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::DoubleColon
                } else {
                    TokenKind::Colon
                }
            }
            ';' => TokenKind::Semi,
            ',' => TokenKind::Comma,
            '.' => {
                if let Some(&(_, '.')) = self.chars.peek() {
                    self.chars.next();
                    if let Some(&(_, '=')) = self.chars.peek() {
                        self.chars.next();
                        TokenKind::DotDotEq
                    } else {
                        TokenKind::DotDot
                    }
                } else {
                    TokenKind::Dot
                }
            }
            '$' => TokenKind::Dollar,
            '?' => {
                if let Some(&(_, '?')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::NullCoalesce
                } else {
                    TokenKind::Question
                }
            }
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            '=' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::EqEq
                } else if let Some(&(_, '>')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::FatArrow
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::BangEq
                } else {
                    TokenKind::Bang
                }
            }
            '~' => TokenKind::Tilde,
            '<' => {
                if let Some(&(_, '<')) = self.chars.peek() {
                    self.chars.next();
                    if let Some(&(_, '=')) = self.chars.peek() {
                        self.chars.next();
                        TokenKind::ShlEq
                    } else {
                        TokenKind::Shl
                    }
                } else if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if let Some(&(_, '>')) = self.chars.peek() {
                    self.chars.next();
                    if let Some(&(_, '=')) = self.chars.peek() {
                        self.chars.next();
                        TokenKind::ShrEq
                    } else {
                        TokenKind::Shr
                    }
                } else if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            '+' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::PlusEq
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                if let Some(&(_, '>')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::Arrow
                } else if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::MinusEq
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                if let Some(&(_, '*')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::StarStar
                } else if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::StarEq
                } else {
                    TokenKind::Star
                }
            }
            '/' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::SlashEq
                } else {
                    TokenKind::Slash
                }
            }
            '%' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::PercentEq
                } else {
                    TokenKind::Percent
                }
            }
            '^' => {
                if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::CaretEq
                } else {
                    TokenKind::Caret
                }
            }
            '&' => {
                if let Some(&(_, '&')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::AmpAmp
                } else if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::AmpEq
                } else {
                    TokenKind::Amp
                }
            }
            '|' => {
                if let Some(&(_, '>')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::Pipeline
                } else if let Some(&(_, '|')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::PipePipe
                } else if let Some(&(_, '=')) = self.chars.peek() {
                    self.chars.next();
                    TokenKind::PipeEq
                } else {
                    TokenKind::Pipe
                }
            }
            '\'' | '"' => {
                let quote = ch;
                let mut string_val = String::new();
                while let Some((_, c)) = self.chars.next() {
                    if c == quote {
                        break;
                    } else if c == '\\' {
                        if let Some((_, esc)) = self.chars.next() {
                            match esc {
                                'n' => string_val.push('\n'),
                                't' => string_val.push('\t'),
                                'r' => string_val.push('\r'),
                                '\\' => string_val.push('\\'),
                                '"' => string_val.push('"'),
                                '\'' => string_val.push('\''),
                                other => string_val.push(other),
                            }
                        }
                    } else {
                        string_val.push(c);
                    }
                }
                TokenKind::Str(string_val)
            }
            '0'..='9' => {
                let mut num_str = String::new();
                num_str.push(ch);
                if ch == '0' {
                    if let Some(&(_, 'x')) | Some(&(_, 'X')) = self.chars.peek() {
                        self.chars.next();
                        let mut hex_str = String::new();
                        while let Some(&(_, h_ch)) = self.chars.peek() {
                            if h_ch.is_ascii_hexdigit() || h_ch == '_' {
                                if h_ch != '_' {
                                    hex_str.push(h_ch);
                                }
                                self.chars.next();
                            } else {
                                break;
                            }
                        }
                        let val = i64::from_str_radix(&hex_str, 16).unwrap_or(0);
                        TokenKind::Int(val)
                    } else {
                        let mut is_float = false;
                        while let Some(&(_, next_ch)) = self.chars.peek() {
                            if next_ch.is_ascii_digit() || next_ch == '_' {
                                if next_ch != '_' {
                                    num_str.push(next_ch);
                                }
                                self.chars.next();
                            } else if next_ch == '.' {
                                let mut clone = self.chars.clone();
                                clone.next();
                                if let Some((_, dot2)) = clone.peek() {
                                    if *dot2 == '.' {
                                        break;
                                    }
                                }
                                if !is_float {
                                    is_float = true;
                                    num_str.push('.');
                                    self.chars.next();
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }

                        if let Some(&(_, unit_ch)) = self.chars.peek() {
                            if unit_ch == 'm' || unit_ch == 'u' || unit_ch == 's' {
                                let mut unit = String::new();
                                while let Some(&(_, u)) = self.chars.peek() {
                                    if u.is_alphabetic() {
                                        unit.push(u);
                                        self.chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                let val: i64 = num_str.parse().unwrap_or(0);
                                let final_val = match unit.as_str() {
                                    "s" => val * 1000,
                                    "ms" => val,
                                    "us" => val / 1000,
                                    _ => val,
                                };
                                TokenKind::Duration(final_val as u64)
                            } else if is_float {
                                TokenKind::Float(num_str)
                            } else {
                                TokenKind::Int(num_str.parse().unwrap_or(0))
                            }
                        } else if is_float {
                            TokenKind::Float(num_str)
                        } else {
                            TokenKind::Int(num_str.parse().unwrap_or(0))
                        }
                    }
                } else {
                    let mut is_float = false;
                    while let Some(&(_, next_ch)) = self.chars.peek() {
                        if next_ch.is_ascii_digit() || next_ch == '_' {
                            if next_ch != '_' {
                                num_str.push(next_ch);
                            }
                            self.chars.next();
                        } else if next_ch == '.' {
                            let mut clone = self.chars.clone();
                            clone.next();
                            if let Some((_, dot2)) = clone.peek() {
                                if *dot2 == '.' {
                                    break;
                                }
                            }
                            if !is_float {
                                is_float = true;
                                num_str.push('.');
                                self.chars.next();
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    if let Some(&(_, unit_ch)) = self.chars.peek() {
                        if unit_ch == 'm' || unit_ch == 'u' || unit_ch == 's' {
                            let mut unit = String::new();
                            while let Some(&(_, u)) = self.chars.peek() {
                                if u.is_alphabetic() {
                                    unit.push(u);
                                    self.chars.next();
                                } else {
                                    break;
                                }
                            }
                            let val: i64 = num_str.parse().unwrap_or(0);
                            let final_val = match unit.as_str() {
                                "s" => val * 1000,
                                "ms" => val,
                                "us" => val / 1000,
                                _ => val,
                            };
                            TokenKind::Duration(final_val as u64)
                        } else if is_float {
                            TokenKind::Float(num_str)
                        } else {
                            TokenKind::Int(num_str.parse().unwrap_or(0))
                        }
                    } else if is_float {
                        TokenKind::Float(num_str)
                    } else {
                        TokenKind::Int(num_str.parse().unwrap_or(0))
                    }
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                if c == 'b' {
                    if let Some(&(_, '"')) | Some(&(_, '\'')) = self.chars.peek() {
                        let (_, quote) = self.chars.next().unwrap();
                        let mut bytes = Vec::new();
                        while let Some((_, b_ch)) = self.chars.next() {
                            if b_ch == quote {
                                break;
                            } else if b_ch == '\\' {
                                if let Some((_, esc)) = self.chars.next() {
                                    match esc {
                                        'n' => bytes.push(b'\n'),
                                        't' => bytes.push(b'\t'),
                                        'r' => bytes.push(b'\r'),
                                        '0' => bytes.push(0),
                                        '\\' => bytes.push(b'\\'),
                                        '"' => bytes.push(b'"'),
                                        '\'' => bytes.push(b'\''),
                                        other => bytes.push(other as u8),
                                    }
                                }
                            } else {
                                bytes.push(b_ch as u8);
                            }
                        }
                        return Token {
                            kind: TokenKind::ByteStr(bytes),
                            span: Span {
                                start,
                                end: self
                                    .chars
                                    .peek()
                                    .map(|&(idx, _)| idx)
                                    .unwrap_or(self.source.len()),
                            },
                        };
                    }
                }
                if c == 'f' {
                    if let Some(&(_, '"')) | Some(&(_, '\'')) = self.chars.peek() {
                        let (_, quote) = self.chars.next().unwrap();
                        let mut string_val = String::new();
                        while let Some((_, f_ch)) = self.chars.next() {
                            if f_ch == quote {
                                break;
                            } else {
                                string_val.push(f_ch);
                            }
                        }
                        return Token {
                            kind: TokenKind::FStr(string_val),
                            span: Span {
                                start,
                                end: self
                                    .chars
                                    .peek()
                                    .map(|&(idx, _)| idx)
                                    .unwrap_or(self.source.len()),
                            },
                        };
                    }
                }

                let mut ident = String::new();
                ident.push(c);
                while let Some(&(_, next_c)) = self.chars.peek() {
                    if next_c.is_alphanumeric() || next_c == '_' {
                        ident.push(next_c);
                        self.chars.next();
                    } else {
                        break;
                    }
                }

                if ident == "hex" {
                    if let Some(&(_, '"')) | Some(&(_, '\'')) = self.chars.peek() {
                        let (_, quote) = self.chars.next().unwrap();
                        let mut hex_content = String::new();
                        while let Some((_, h_c)) = self.chars.next() {
                            if h_c == quote {
                                break;
                            } else if !h_c.is_whitespace() {
                                hex_content.push(h_c);
                            }
                        }
                        let mut bytes = Vec::new();
                        let chars: Vec<char> = hex_content.chars().collect();
                        let mut i = 0;
                        while i + 1 < chars.len() {
                            let chunk: String = chars[i..i + 2].iter().collect();
                            if let Ok(b) = u8::from_str_radix(&chunk, 16) {
                                bytes.push(b);
                            }
                            i += 2;
                        }
                        return Token {
                            kind: TokenKind::HexByteStr(bytes),
                            span: Span {
                                start,
                                end: self
                                    .chars
                                    .peek()
                                    .map(|&(idx, _)| idx)
                                    .unwrap_or(self.source.len()),
                            },
                        };
                    }
                }

                match ident.as_str() {
                    "let" => TokenKind::Let,
                    "mut" => TokenKind::Mut,
                    "pub" => TokenKind::Pub,
                    "yield" => TokenKind::Yield,
                    "return" => TokenKind::Return,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "match" => TokenKind::Match,
                    "loop" => TokenKind::Loop,
                    "while" => TokenKind::While,
                    "break" => TokenKind::Break,
                    "continue" => TokenKind::Continue,
                    "routine" => TokenKind::Routine,
                    "taking" => TokenKind::Taking,
                    "actor" => TokenKind::Actor,
                    "require" | "requires" => TokenKind::Require,
                    "on" => TokenKind::On,
                    "send" => TokenKind::Send,
                    "to" => TokenKind::To,
                    "isolate" => TokenKind::Isolate,
                    "enable" => TokenKind::Enable,
                    "lease" => TokenKind::Lease,
                    "for" => TokenKind::For,
                    "in" => TokenKind::In,
                    "step" => TokenKind::Step,
                    "import" => TokenKind::Import,
                    "from" => TokenKind::From,
                    "as" => TokenKind::As,
                    "type" => TokenKind::Type,
                    "struct" => TokenKind::Struct,
                    "enum" => TokenKind::Enum,
                    "interface" => TokenKind::Interface,
                    "using" => TokenKind::Using,
                    "reconcile" => TokenKind::Reconcile,
                    "speculate" => TokenKind::Speculate,
                    "commit" => TokenKind::Commit,
                    "fallback" => TokenKind::Fallback,
                    "collapse" => TokenKind::Collapse,
                    "anchor" => TokenKind::Anchor,
                    "rewind_to" => TokenKind::RewindTo,
                    "entangle" => TokenKind::Entangle,
                    "assert_time" => TokenKind::AssertTime,
                    "print" => TokenKind::Print,
                    "tick" => TokenKind::Tick,
                    "max" => TokenKind::Max,
                    "foreign" => TokenKind::Foreign,
                    "macro" => TokenKind::Macro,
                    "slice" => TokenKind::Slice,
                    "state" => TokenKind::State,
                    "policy" => TokenKind::Policy,
                    "select" => TokenKind::Select,
                    "case" => TokenKind::Case,
                    "timeout" => TokenKind::Timeout,
                    "Valid" => TokenKind::Valid,
                    "Decayed" => TokenKind::Decayed,
                    "Pending" => TokenKind::Pending,
                    "Consumed" => TokenKind::Consumed,
                    "defer" => TokenKind::Defer,
                    "chan_recv" => TokenKind::ChanRecv,
                    "syscall" => TokenKind::Syscall,
                    "arena" => TokenKind::Arena,
                    "distinct" => TokenKind::Distinct,
                    "split" => TokenKind::Split,
                    "merge" => TokenKind::Merge,
                    "into" => TokenKind::Into,
                    "auto" => TokenKind::Auto,
                    "debug" => TokenKind::Debug,
                    "log" => TokenKind::Log,
                    "on_decay" => TokenKind::OnDecay,
                    "where" => TokenKind::Where,
                    "true" => TokenKind::Bool(true),
                    "false" => TokenKind::Bool(false),
                    "null" => TokenKind::Null,
                    _ => TokenKind::Ident(intern(&ident)),
                }
            }
            _ => TokenKind::Ident(intern(&ch.to_string())),
        };

        let end = self
            .chars
            .peek()
            .map(|&(idx, _)| idx)
            .unwrap_or_else(|| self.source.len());

        Token {
            kind,
            span: Span { start, end },
        }
    }
}
