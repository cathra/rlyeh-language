//! Zeta 语言词法分析器
//!
//! 将源代码字符串转换为 Token 流，供语法分析器使用。
//!
//! # 示例
//! ```
//! use zeta_lexer::{Lexer, Token};
//!
//! let source = "let x = 42;";
//! let mut lexer = Lexer::new(source);
//! let tokens = lexer.tokenize().unwrap();
//! assert_eq!(tokens.len(), 5); // let, x, =, 42, ;
//! ```

#![warn(missing_docs)]
#![warn(unsafe_code)]

mod error;
mod token;

pub use error::LexError;
pub use token::{LocatedToken, Span, Token};

use unicode_xid::UnicodeXID;

#[cfg(test)]
mod tests;

/// 词法分析器
pub struct Lexer<'src> {
    source: &'src str,
    /// 当前消费到的字节偏移
    byte_pos: usize,
    /// 当前行号（从 1 开始）
    line: usize,
    /// 当前列号（从 1 开始，按字符计）
    col: usize,
    /// `peek()` 的 1 token lookahead 缓存
    buffered: Option<LocatedToken>,
}

impl<'src> Lexer<'src> {
    /// 创建新的词法分析器
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            byte_pos: 0,
            line: 1,
            col: 1,
            buffered: None,
        }
    }

    /// 执行词法分析，返回所有 Token（不含 Eof）
    pub fn tokenize(&mut self) -> Result<Vec<LocatedToken>, LexError> {
        let mut tokens = Vec::new();
        while let Some(t) = self.next_token()? {
            if t.token == Token::Eof {
                break;
            }
            tokens.push(t);
        }
        Ok(tokens)
    }

    /// 查看下一个 Token 但不消费（支持 1 token 的 lookahead）
    pub fn peek(&mut self) -> Result<&LocatedToken, LexError> {
        if self.buffered.is_none() {
            let t = match self.next_token_raw()? {
                Some(t) => t,
                None => LocatedToken::new(
                    Token::Eof,
                    Span {
                        start: self.byte_pos,
                        end: self.byte_pos,
                        line: self.line,
                        col: self.col,
                    },
                ),
            };
            self.buffered = Some(t);
        }
        Ok(self.buffered.as_ref().unwrap())
    }

    // ===== 内部驱动 =====

    /// 取下一个 Token（优先消费 peek 缓存），EOF 时返回 None
    fn next_token(&mut self) -> Result<Option<LocatedToken>, LexError> {
        if let Some(t) = self.buffered.take() {
            return Ok(Some(t));
        }
        self.next_token_raw()
    }

    /// 真正生成下一个 Token
    fn next_token_raw(&mut self) -> Result<Option<LocatedToken>, LexError> {
        self.skip_whitespace_and_comments()?;
        let (line, col) = (self.line, self.col);
        let start = self.byte_pos;

        let c = match self.peek_char() {
            Some(c) => c,
            None => return Ok(None),
        };

        let token = match c {
            c if c.is_ascii_digit() => self.read_number_or_time()?,
            '"' => self.read_string()?,
            '\'' => self.read_char_or_lifetime()?,
            // 原始字符串 / 原始标识符：必须在普通标识符分支之前判断
            'r' if self.peek_nth(1) == Some('"') => self.read_raw_string()?,
            'r' if self.peek_nth(1) == Some('#') => self.read_raw_ident(),
            c if c.is_xid_start() || c == '_' => self.read_identifier(),
            '@' => {
                self.bump();
                Token::At
            }
            '(' | ')' | '{' | '}' | '[' | ']' | ',' | ':' | ';' | '.' | '+' | '-' | '*' | '/'
            | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' => self.read_operator(),
            c => return Err(LexError::InvalidChar { ch: c, line, col }),
        };

        let span = Span {
            start,
            end: self.byte_pos,
            line,
            col,
        };
        Ok(Some(LocatedToken::new(token, span)))
    }

    // ===== 字符辅助 =====

    /// 当前字符
    fn peek_char(&self) -> Option<char> {
        self.source[self.byte_pos..].chars().next()
    }

    /// 当前字符之后第 n 个字符（n=1 为下一个）
    fn peek_nth(&self, n: usize) -> Option<char> {
        self.source[self.byte_pos..].chars().nth(n)
    }

    /// 当前剩余源码是否以指定字符串开头
    fn starts_with(&self, s: &str) -> bool {
        self.source[self.byte_pos..].starts_with(s)
    }

    /// 消费一个字符并更新行列
    fn bump(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.byte_pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    /// 跳过空白与注释（单行 `//`、嵌套块注释 `/* */`）
    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek_char() {
                Some(' ') | Some('\t') | Some('\r') | Some('\n') => {
                    self.bump();
                }
                Some('/') if self.peek_nth(1) == Some('/') => {
                    while let Some(c) = self.peek_char() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.peek_nth(1) == Some('*') => {
                    let (line, col) = (self.line, self.col);
                    self.bump();
                    self.bump();
                    let mut depth = 1usize;
                    while depth > 0 {
                        match self.peek_char() {
                            None => return Err(LexError::UnterminatedBlockComment { line, col }),
                            Some('/') if self.peek_nth(1) == Some('*') => {
                                self.bump();
                                self.bump();
                                depth += 1;
                            }
                            Some('*') if self.peek_nth(1) == Some('/') => {
                                self.bump();
                                self.bump();
                                depth -= 1;
                            }
                            Some(_) => {
                                self.bump();
                            }
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// 只跳过空白（`not in` 组合判断用，不回退注释）
    fn skip_plain_whitespace(&mut self) {
        while matches!(
            self.peek_char(),
            Some(' ') | Some('\t') | Some('\r') | Some('\n')
        ) {
            self.bump();
        }
    }

    // ===== 标识符与关键字 =====

    /// 读取标识符或关键字；`not` 后跟 `in` 时合并为 `NotIn`
    fn read_identifier(&mut self) -> Token {
        let text = self.read_ident_text();
        if text == "not" {
            let saved = (self.byte_pos, self.line, self.col);
            self.skip_plain_whitespace();
            if self.starts_with("in") && !matches!(self.peek_nth(2), Some(c) if c.is_xid_continue())
            {
                self.bump();
                self.bump();
                return Token::NotIn;
            }
            (self.byte_pos, self.line, self.col) = saved;
            return Token::Not;
        }
        keyword_or_ident(&text)
    }

    /// 读取标识符文本（不含起始字符校验）
    fn read_ident_text(&mut self) -> String {
        let start = self.byte_pos;
        while let Some(c) = self.peek_char() {
            if c.is_xid_continue() {
                self.bump();
            } else {
                break;
            }
        }
        self.source[start..self.byte_pos].to_string()
    }

    /// 原始标识符 `r#keyword` → `Ident("keyword")`
    fn read_raw_ident(&mut self) -> Token {
        self.bump(); // r
        self.bump(); // #
        let text = self.read_ident_text();
        Token::Ident(text)
    }

    // ===== 数字与时间字面量 =====

    /// 读取数字字面量（整数/浮点/时间）
    fn read_number_or_time(&mut self) -> Result<Token, LexError> {
        let (line, col) = (self.line, self.col);

        // 前缀进制
        if self.peek_char() == Some('0') {
            match self.peek_nth(1) {
                Some('x') => return self.read_radix(16),
                Some('b') => return self.read_radix(2),
                Some('o') => return self.read_radix(8),
                _ => {}
            }
        }

        // 十进制整数部分（支持下划线）
        let mut digits = String::new();
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() || c == '_' {
                digits.push(c);
                self.bump();
            } else {
                break;
            }
        }

        // 时间字面量：9am / 6pm
        let is_am = self.peek_char() == Some('a')
            && self.peek_nth(1) == Some('m')
            && !matches!(self.peek_nth(2), Some(c) if c.is_xid_continue());
        let is_pm = self.peek_char() == Some('p')
            && self.peek_nth(1) == Some('m')
            && !matches!(self.peek_nth(2), Some(c) if c.is_xid_continue());
        if is_am || is_pm {
            self.bump();
            self.bump();
            let hour_raw: i32 = digits
                .replace('_', "")
                .parse()
                .map_err(|_| LexError::InvalidTime { line, col })?;
            let hour = normalize_hour(hour_raw, is_pm);
            if !(0..=23).contains(&hour) {
                return Err(LexError::InvalidTime { line, col });
            }
            return Ok(Token::TimeLiteral {
                hour: hour as u8,
                minute: 0,
                is_pm,
            });
        }

        // 时间字面量：22:00 / 09:30am
        if self.peek_char() == Some(':') {
            self.bump(); // :
            let mut mins = String::new();
            while let Some(c) = self.peek_char() {
                if c.is_ascii_digit() {
                    mins.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
            let mut is_pm = false;
            if self.peek_char() == Some('a')
                && self.peek_nth(1) == Some('m')
                && !matches!(self.peek_nth(2), Some(c) if c.is_xid_continue())
            {
                self.bump();
                self.bump();
            } else if self.peek_char() == Some('p')
                && self.peek_nth(1) == Some('m')
                && !matches!(self.peek_nth(2), Some(c) if c.is_xid_continue())
            {
                self.bump();
                self.bump();
                is_pm = true;
            }
            let hour_raw: i32 = digits
                .replace('_', "")
                .parse()
                .map_err(|_| LexError::InvalidTime { line, col })?;
            let minute: i32 = mins
                .parse()
                .map_err(|_| LexError::InvalidTime { line, col })?;
            if !(0..=23).contains(&hour_raw) || !(0..=59).contains(&minute) {
                return Err(LexError::InvalidTime { line, col });
            }
            let hour = normalize_hour(hour_raw, is_pm);
            return Ok(Token::TimeLiteral {
                hour: hour as u8,
                minute: minute as u8,
                is_pm,
            });
        }

        // 浮点：小数部分
        if self.peek_char() == Some('.')
            && matches!(self.peek_nth(1), Some(c) if c.is_ascii_digit())
        {
            digits.push('.');
            self.bump();
            while let Some(c) = self.peek_char() {
                if c.is_ascii_digit() || c == '_' {
                    digits.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
        }

        // 浮点：指数部分（1e10 / 2.5e-10）
        let exp_direct = matches!(self.peek_nth(1), Some(c) if c.is_ascii_digit());
        let exp_signed = matches!(self.peek_nth(1), Some('+') | Some('-'))
            && matches!(self.peek_nth(2), Some(c) if c.is_ascii_digit());
        if matches!(self.peek_char(), Some('e') | Some('E')) && (exp_direct || exp_signed) {
            digits.push('e');
            self.bump(); // e
            if matches!(self.peek_char(), Some('+') | Some('-')) {
                digits.push(self.peek_char().unwrap());
                self.bump();
            }
            while let Some(c) = self.peek_char() {
                if c.is_ascii_digit() || c == '_' {
                    digits.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
        }

        // 类型后缀：42i32 / 3.14f64（吸收但不改变 Token 值）
        if let Some(c) = self.peek_char() {
            if c.is_xid_start() || c == '_' {
                self.read_ident_text();
            }
        }

        // 解析
        let clean: String = digits.chars().filter(|c| *c != '_').collect();
        if digits.contains('.') || digits.contains('e') || digits.contains('E') {
            match clean.parse::<f64>() {
                Ok(v) => Ok(Token::FloatLiteral(v)),
                Err(_) => Err(LexError::IntTooLarge { line, col }),
            }
        } else {
            match clean.parse::<i128>() {
                Ok(v) => Ok(Token::IntLiteral(v)),
                Err(_) => Err(LexError::IntTooLarge { line, col }),
            }
        }
    }

    /// 读取指定进制整数（0x / 0b / 0o）
    fn read_radix(&mut self, radix: u32) -> Result<Token, LexError> {
        let (line, col) = (self.line, self.col);
        self.bump(); // 0
        self.bump(); // 前缀字符
        let mut digits = String::new();
        while let Some(c) = self.peek_char() {
            if c.is_digit(radix) || c == '_' {
                digits.push(c);
                self.bump();
            } else {
                break;
            }
        }
        let clean: String = digits.chars().filter(|c| *c != '_').collect();
        match i128::from_str_radix(&clean, radix) {
            Ok(v) => Ok(Token::IntLiteral(v)),
            Err(_) => Err(LexError::IntTooLarge { line, col }),
        }
    }

    // ===== 字符串与字符 =====

    /// 读取字符串字面量（支持转义、Unicode、多行）
    fn read_string(&mut self) -> Result<Token, LexError> {
        let (line, col) = (self.line, self.col);
        self.bump(); // "
        let mut result = String::new();
        loop {
            match self.peek_char() {
                None => return Err(LexError::UnterminatedString { line, col }),
                Some('"') => {
                    self.bump();
                    return Ok(Token::StringLiteral(result));
                }
                Some('\\') => {
                    self.bump(); // \
                    result.push(self.read_escape(line, col)?);
                }
                Some(c) => {
                    result.push(c);
                    self.bump();
                }
            }
        }
    }

    /// 读取原始字符串 `r"..."`（不处理转义）
    fn read_raw_string(&mut self) -> Result<Token, LexError> {
        let (line, col) = (self.line, self.col);
        self.bump(); // r
        self.bump(); // "
        let start = self.byte_pos;
        while let Some(c) = self.peek_char() {
            if c == '"' {
                let text = self.source[start..self.byte_pos].to_string();
                self.bump();
                return Ok(Token::StringLiteral(text));
            }
            self.bump();
        }
        Err(LexError::UnterminatedString { line, col })
    }

    /// 判断是字符字面量还是生命周期标签，并分发
    fn read_char_or_lifetime(&mut self) -> Result<Token, LexError> {
        let after = self.peek_nth(1);
        let after2 = self.peek_nth(2);
        if let Some(c) = after {
            if (c.is_xid_start() || c == '_') && after2 != Some('\'') {
                return Ok(self.read_lifetime());
            }
        }
        self.read_char()
    }

    /// 生命周期/区域标签 `'r` → `Lifetime("r")`
    fn read_lifetime(&mut self) -> Token {
        self.bump(); // '
        let text = self.read_ident_text();
        Token::Lifetime(text)
    }

    /// 读取字符字面量（支持转义）
    fn read_char(&mut self) -> Result<Token, LexError> {
        let (line, col) = (self.line, self.col);
        self.bump(); // '
        let c = match self.peek_char() {
            None => return Err(LexError::UnterminatedChar { line, col }),
            Some('\\') => {
                self.bump(); // \
                self.read_escape(line, col)?
            }
            Some(c) => {
                self.bump();
                c
            }
        };
        if self.peek_char() == Some('\'') {
            self.bump();
            Ok(Token::CharLiteral(c))
        } else {
            Err(LexError::UnterminatedChar { line, col })
        }
    }

    /// 读取转义序列（`\n`、`\t`、`\xHH`、`\u{...}` 等），返回转义后的字符
    fn read_escape(&mut self, line: usize, col: usize) -> Result<char, LexError> {
        let esc = match self.peek_char() {
            None => return Err(LexError::UnterminatedString { line, col }),
            Some(c) => c,
        };
        match esc {
            'n' => {
                self.bump();
                Ok('\n')
            }
            't' => {
                self.bump();
                Ok('\t')
            }
            'r' => {
                self.bump();
                Ok('\r')
            }
            '\\' => {
                self.bump();
                Ok('\\')
            }
            '"' => {
                self.bump();
                Ok('"')
            }
            '\'' => {
                self.bump();
                Ok('\'')
            }
            'x' => {
                self.bump();
                let mut hex = String::new();
                for _ in 0..2 {
                    match self.peek_char() {
                        Some(c) if c.is_ascii_hexdigit() => {
                            hex.push(c);
                            self.bump();
                        }
                        _ => {
                            return Err(LexError::InvalidEscape {
                                ch: 'x',
                                line: self.line,
                                col: self.col,
                            })
                        }
                    }
                }
                let v = u32::from_str_radix(&hex, 16).unwrap();
                Ok(char::from_u32(v).unwrap_or('\u{FFFD}'))
            }
            'u' => {
                self.bump();
                if self.peek_char() != Some('{') {
                    return Err(LexError::InvalidEscape {
                        ch: 'u',
                        line: self.line,
                        col: self.col,
                    });
                }
                self.bump(); // {
                let mut hex = String::new();
                while let Some(c) = self.peek_char() {
                    if c == '}' {
                        break;
                    }
                    if c.is_ascii_hexdigit() {
                        hex.push(c);
                        self.bump();
                    } else {
                        return Err(LexError::InvalidEscape {
                            ch: 'u',
                            line: self.line,
                            col: self.col,
                        });
                    }
                }
                if self.peek_char() != Some('}') {
                    return Err(LexError::InvalidEscape {
                        ch: 'u',
                        line: self.line,
                        col: self.col,
                    });
                }
                self.bump(); // }
                let v = u32::from_str_radix(&hex, 16).map_err(|_| LexError::InvalidEscape {
                    ch: 'u',
                    line: self.line,
                    col: self.col,
                })?;
                Ok(char::from_u32(v).unwrap_or('\u{FFFD}'))
            }
            other => Err(LexError::InvalidEscape {
                ch: other,
                line: self.line,
                col: self.col,
            }),
        }
    }

    // ===== 运算符 =====

    /// 读取运算符与分隔符（支持多字符运算符）
    fn read_operator(&mut self) -> Token {
        let c = self.peek_char().expect("operator needs a char");
        let c2 = self.peek_nth(1);
        match (c, c2) {
            ('.', Some('.')) => {
                self.bump();
                self.bump();
                match self.peek_char() {
                    // `...` 闭区间
                    Some('.') => {
                        self.bump();
                        Token::DotDotDot
                    }
                    // `..<` 左闭右开
                    Some('<') => {
                        self.bump();
                        Token::DotDotLt
                    }
                    // `..=` 旧语法已废弃：返回 Range，`=` 留待主循环 lex 为 Assign
                    Some('=') => Token::Range,
                    _ => Token::Range,
                }
            }
            ('+', Some('=')) => {
                self.bump();
                self.bump();
                Token::PlusEq
            }
            ('-', Some('>')) => {
                self.bump();
                self.bump();
                Token::Arrow
            }
            ('-', Some('=')) => {
                self.bump();
                self.bump();
                Token::MinusEq
            }
            ('*', Some('=')) => {
                self.bump();
                self.bump();
                Token::StarEq
            }
            ('/', Some('=')) => {
                self.bump();
                self.bump();
                Token::SlashEq
            }
            ('%', Some('=')) => {
                self.bump();
                self.bump();
                Token::PercentEq
            }
            ('=', Some('=')) => {
                self.bump();
                self.bump();
                Token::Eq
            }
            ('=', Some('>')) => {
                self.bump();
                self.bump();
                Token::FatArrow
            }
            ('!', Some('=')) => {
                self.bump();
                self.bump();
                Token::Ne
            }
            ('<', Some('=')) => {
                self.bump();
                self.bump();
                Token::Le
            }
            // `<..` 左开右闭区间（需完整匹配 `<` + `..` 三个字符）
            ('<', Some('.')) if self.peek_nth(2) == Some('.') => {
                self.bump();
                self.bump();
                self.bump();
                Token::LtDotDot
            }
            ('<', Some('<')) => {
                self.bump();
                self.bump();
                Token::Shl
            }
            ('>', Some('=')) => {
                self.bump();
                self.bump();
                Token::Ge
            }
            ('>', Some('>')) => {
                self.bump();
                self.bump();
                Token::Shr
            }
            ('&', Some('&')) => {
                self.bump();
                self.bump();
                Token::AndAnd
            }
            ('|', Some('|')) => {
                self.bump();
                self.bump();
                Token::OrOr
            }
            (c, _) => {
                self.bump();
                match c {
                    '+' => Token::Plus,
                    '-' => Token::Minus,
                    '*' => Token::Star,
                    '/' => Token::Slash,
                    '%' => Token::Percent,
                    '=' => Token::Assign,
                    '<' => Token::Lt,
                    '>' => Token::Gt,
                    '!' => Token::NotNot,
                    '&' => Token::BitAnd,
                    '|' => Token::BitOr,
                    '^' => Token::BitXor,
                    '(' => Token::LParen,
                    ')' => Token::RParen,
                    '{' => Token::LBrace,
                    '}' => Token::RBrace,
                    '[' => Token::LBracket,
                    ']' => Token::RBracket,
                    ',' => Token::Comma,
                    ':' => Token::Colon,
                    ';' => Token::Semicolon,
                    '.' => Token::Dot,
                    '@' => Token::At,
                    // 主循环已过滤非法字符，此处不可达
                    other => unreachable!("unexpected char {other:?} in read_operator"),
                }
            }
        }
    }
}

/// 关键字表：将标识符文本映射为关键字 Token
fn keyword_or_ident(text: &str) -> Token {
    match text {
        "let" => Token::Let,
        "mut" => Token::Mut,
        "const" => Token::Const,
        "static" => Token::Static,
        "fn" => Token::Fn,
        "return" => Token::Return,
        "pub" => Token::Pub,
        "priv" => Token::Priv,
        "if" => Token::If,
        "else" => Token::Else,
        "match" => Token::Match,
        "for" => Token::For,
        "while" => Token::While,
        "loop" => Token::Loop,
        "break" => Token::Break,
        "continue" => Token::Continue,
        "true" => Token::True,
        "false" => Token::False,
        "and" => Token::And,
        "or" => Token::Or,
        "not" => Token::Not,
        "struct" => Token::Struct,
        "enum" => Token::Enum,
        "trait" => Token::Trait,
        "impl" => Token::Impl,
        "type" => Token::Type,
        "where" => Token::Where,
        "Self" => Token::SelfKw,
        "region" => Token::Region,
        "in" => Token::In,
        "transfer" => Token::Transfer,
        "out" => Token::Out,
        "of" => Token::Of,
        "unsafe" => Token::Unsafe,
        "actor" => Token::Actor,
        "async" => Token::Async,
        "await" => Token::Await,
        "spawn" => Token::Spawn,
        "send" => Token::Send,
        "recv" => Token::Recv,
        "mod" => Token::Mod,
        "use" => Token::Use,
        "as" => Token::As,
        "extern" => Token::Extern,
        _ => Token::Ident(text.to_string()),
    }
}

/// 12 小时制转换：pm +12（12pm→12），am 不变（12am→0）
fn normalize_hour(hour: i32, is_pm: bool) -> i32 {
    if is_pm {
        if hour == 12 {
            12
        } else {
            hour + 12
        }
    } else if hour == 12 {
        0
    } else {
        hour
    }
}
