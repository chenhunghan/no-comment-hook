//! Finds the comments in a source file without a parser: one forward scan that
//! knows each language's strings well enough never to mistake their contents
//! for a comment. Strings that interpolate code (`${}`, `#{}`, `\()`, f-strings)
//! are scanned recursively, since the embedded code can hold strings and
//! comments of its own.

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Line,
    Block,
    DocString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Comment {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lang {
    Rust,
    C,
    Cpp,
    Go,
    Java,
    Kotlin,
    Scala,
    Swift,
    CSharp,
    Jsx,
    Ts,
    Python,
    Ruby,
    Php,
    Generic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Interp {
    None,
    /// `${ … }`
    Dollar,
    /// `#{ … }`
    Hash,
    /// `{ … }`, with `{{` as a literal brace
    Brace,
    /// `{$ … }`
    PhpBrace,
    /// `\( … )`
    Paren,
}

fn lang_of(file_path: &str) -> Lang {
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "rs" => Lang::Rust,
        "c" | "h" => Lang::C,
        "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hh" | "hxx" | "h++" => Lang::Cpp,
        "go" => Lang::Go,
        "java" => Lang::Java,
        "kt" | "kts" => Lang::Kotlin,
        "scala" | "sc" => Lang::Scala,
        "swift" => Lang::Swift,
        "cs" => Lang::CSharp,
        "js" | "mjs" | "cjs" | "jsx" | "tsx" => Lang::Jsx,
        "ts" | "mts" | "cts" => Lang::Ts,
        "py" | "pyi" => Lang::Python,
        "rb" | "rake" | "gemspec" => Lang::Ruby,
        "php" => Lang::Php,
        _ => Lang::Generic,
    }
}

/// Every comment in `src`, in source order, located by byte offsets.
pub fn comments(src: &str, file_path: &str) -> Vec<Comment> {
    let mut lx = Lexer::new(src.as_bytes(), lang_of(file_path));
    if lx.lang == Lang::Php {
        lx.php_html();
    }
    lx.code(None);
    lx.out
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

const REGEX_KEYWORDS: &[&[u8]] = &[
    b"return", b"typeof", b"case", b"do", b"else", b"in", b"of", b"new", b"delete", b"void",
    b"throw", b"yield", b"await", b"when", b"if", b"unless", b"and", b"or", b"not", b"then",
    b"elsif", b"while", b"until",
];

const CPP_RAW_PREFIXES: [&[u8]; 5] = [b"u8R\"", b"uR\"", b"UR\"", b"LR\"", b"R\""];

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
    lang: Lang,
    out: Vec<Comment>,
    /// Last significant token class: `a` word, `0` number, `"` literal, else the punctuation byte.
    last: Option<u8>,
    last_word: (usize, usize),
    depth: i32,
    /// Inside a Python f-string field, where `#` (as in `{x:#x}`) is not a comment.
    in_fstring: u32,
    heredocs: Vec<Vec<u8>>,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a [u8], lang: Lang) -> Self {
        Lexer {
            s,
            i: 0,
            lang,
            out: Vec::new(),
            last: None,
            last_word: (0, 0),
            depth: 0,
            in_fstring: 0,
            heredocs: Vec::new(),
        }
    }

    fn at(&self, k: usize) -> u8 {
        self.s.get(self.i + k).copied().unwrap_or(0)
    }

    fn starts(&self, p: &[u8]) -> bool {
        self.s[self.i.min(self.s.len())..].starts_with(p)
    }

    /// How many `b` bytes run from offset `k`.
    fn run_of(&self, k: usize, b: u8) -> usize {
        self.s[(self.i + k).min(self.s.len())..]
            .iter()
            .take_while(|&&c| c == b)
            .count()
    }

    fn done(&self) -> bool {
        self.i >= self.s.len()
    }

    fn push(&mut self, start: usize, kind: Kind) {
        let end = self.i.min(self.s.len());
        self.out.push(Comment { start, end, kind });
    }

    fn literal(&mut self) {
        self.last = Some(b'"');
    }

    fn prev_is_ident(&self) -> bool {
        self.i > 0 && is_ident(self.s[self.i - 1])
    }

    fn only_ws_before_on_line(&self, pos: usize) -> bool {
        self.s[..pos]
            .iter()
            .rev()
            .take_while(|&&c| c != b'\n')
            .all(|&c| c == b' ' || c == b'\t')
    }

    fn slash_comments(&self) -> bool {
        !matches!(self.lang, Lang::Python | Lang::Ruby)
    }

    fn hash_comments(&self) -> bool {
        matches!(
            self.lang,
            Lang::Python | Lang::Ruby | Lang::Php | Lang::Generic
        ) && self.in_fstring == 0
    }

    fn nested_blocks(&self) -> bool {
        matches!(
            self.lang,
            Lang::Rust | Lang::Swift | Lang::Kotlin | Lang::Scala
        )
    }

    fn regex_allowed(&self) -> bool {
        match self.last {
            None => true,
            Some(b'a') => {
                let (a, b) = self.last_word;
                REGEX_KEYWORDS.contains(&&self.s[a..b])
            }
            Some(c) => b"(,=:[!&|?{};+-*%<>~^".contains(&c),
        }
    }

    // ------------------------------------------------------------- code

    /// Scans code until the end of input, or until the `close` bracket that
    /// matches an interpolation opened by the caller.
    fn code(&mut self, close: Option<u8>) {
        let open = match close {
            Some(b'}') => Some(b'{'),
            Some(b')') => Some(b'('),
            _ => None,
        };
        let mut nest = 0;
        while !self.done() {
            let c = self.s[self.i];
            if close.is_some() && Some(c) == open {
                nest += 1;
            } else if close == Some(c) {
                if nest == 0 {
                    self.i += 1;
                    return;
                }
                nest -= 1;
            }
            if self.lang == Lang::Php && self.starts(b"?>") {
                self.i += 2;
                self.php_html();
                continue;
            }
            if self.lang == Lang::Ruby
                && self.starts(b"__END__")
                && self.only_ws_before_on_line(self.i)
            {
                self.i = self.s.len();
                return;
            }
            if self.comment() || self.string() {
                continue;
            }
            if c == b'\n' && !self.heredocs.is_empty() {
                self.ruby_heredoc_bodies();
                continue;
            }
            self.token(c);
        }
    }

    fn token(&mut self, c: u8) {
        if c.is_ascii_whitespace() {
            self.i += 1;
        } else if c.is_ascii_digit() {
            let digit_sep = matches!(self.lang, Lang::C | Lang::Cpp);
            while !self.done()
                && (is_ident(self.s[self.i])
                    || self.s[self.i] == b'.'
                    || (digit_sep && self.s[self.i] == b'\''))
            {
                self.i += 1;
            }
            self.last = Some(b'0');
        } else if is_ident(c) {
            let start = self.i;
            while !self.done() && is_ident(self.s[self.i]) {
                self.i += 1;
            }
            self.last_word = (start, self.i);
            self.last = Some(b'a');
        } else {
            match c {
                b'(' | b'[' | b'{' => self.depth += 1,
                b')' | b']' | b'}' => self.depth -= 1,
                _ => {}
            }
            self.last = Some(c);
            self.i += 1;
        }
    }

    // ---------------------------------------------------------- comments

    fn comment(&mut self) -> bool {
        let c = self.at(0);
        if self.lang == Lang::Ruby
            && c == b'='
            && self.starts(b"=begin")
            && (self.i == 0 || self.s[self.i - 1] == b'\n')
        {
            let start = self.i;
            while !self.done()
                && (self.s[self.i] != b'\n' || !self.s[self.i + 1..].starts_with(b"=end"))
            {
                self.i += 1;
            }
            self.i += 1;
            self.skip_to_eol();
            self.push(start, Kind::Block);
            return true;
        }
        if self.slash_comments() && c == b'/' && self.at(1) == b'*' {
            let start = self.i;
            self.block_comment();
            self.push(start, Kind::Block);
            return true;
        }
        let line = (self.slash_comments() && c == b'/' && self.at(1) == b'/')
            || (self.hash_comments()
                && c == b'#'
                && !(self.lang == Lang::Php && self.at(1) == b'['))
            || (self.lang == Lang::Generic && self.starts(b"-- "));
        if line {
            let start = self.i;
            self.skip_to_eol();
            self.push(start, Kind::Line);
            return true;
        }
        false
    }

    fn block_comment(&mut self) {
        self.i += 2;
        let mut nest = 1;
        while !self.done() {
            if self.starts(b"*/") {
                self.i += 2;
                nest -= 1;
                if nest == 0 || !self.nested_blocks() {
                    return;
                }
            } else if self.nested_blocks() && self.starts(b"/*") {
                self.i += 2;
                nest += 1;
            } else {
                self.i += 1;
            }
        }
    }

    fn skip_to_eol(&mut self) {
        while !self.done() && self.s[self.i] != b'\n' {
            if self.lang == Lang::Php && self.starts(b"?>") {
                return;
            }
            self.i += 1;
        }
    }

    // ----------------------------------------------------------- strings

    fn string(&mut self) -> bool {
        match self.lang {
            Lang::Python => self.python_string(),
            Lang::Ruby => self.ruby_literal(),
            Lang::Generic => self.generic_string(),
            _ => {
                self.prefixed_string()
                    || self.quoted()
                    || self.backtick()
                    || self.regex()
                    || self.php_heredoc()
                    || self.jsx()
            }
        }
    }

    /// Skips a quoted body; `self.i` is just past the opening delimiter.
    fn body(&mut self, end: &[u8], escapes: bool, interp: Interp) {
        while !self.done() {
            if self.starts(end) {
                self.i += end.len();
                return;
            }
            if self.interpolation(interp) {
                continue;
            }
            if escapes && self.s[self.i] == b'\\' {
                self.i += 2;
                continue;
            }
            self.i += 1;
        }
    }

    fn interpolation(&mut self, interp: Interp) -> bool {
        let (opener, close): (&[u8], u8) = match interp {
            Interp::None => return false,
            Interp::Dollar => (b"${", b'}'),
            Interp::Hash => (b"#{", b'}'),
            Interp::PhpBrace => (b"{$", b'}'),
            Interp::Paren => (b"\\(", b')'),
            Interp::Brace => {
                if self.starts(b"{{") {
                    self.i += 2;
                    return true;
                }
                (b"{", b'}')
            }
        };
        if !self.starts(opener) {
            return false;
        }
        self.i += opener.len();
        if interp == Interp::PhpBrace {
            self.i -= 1;
        }
        let (last, word) = (self.last, self.last_word);
        self.last = None;
        let fstring = self.lang == Lang::Python;
        self.in_fstring += u32::from(fstring);
        self.code(Some(close));
        self.in_fstring -= u32::from(fstring);
        (self.last, self.last_word) = (last, word);
        true
    }

    fn quoted(&mut self) -> bool {
        let c = self.at(0);
        if c == b'"' {
            self.double_quoted();
            return true;
        }
        if c == b'\'' {
            return self.single_quoted();
        }
        false
    }

    fn double_quoted(&mut self) {
        let triple = self.starts(b"\"\"\"");
        let interp = match self.lang {
            Lang::Kotlin => Interp::Dollar,
            Lang::Swift => Interp::Paren,
            Lang::Php => Interp::PhpBrace,
            Lang::Scala if self.prev_is_ident() => Interp::Dollar,
            _ => Interp::None,
        };
        if triple
            && matches!(
                self.lang,
                Lang::Java | Lang::Kotlin | Lang::Scala | Lang::Swift
            )
        {
            self.i += 3;
            let escapes = matches!(self.lang, Lang::Java | Lang::Swift);
            self.body(b"\"\"\"", escapes, interp);
            while self.at(0) == b'"' {
                self.i += 1;
            }
        } else {
            self.i += 1;
            self.body(b"\"", true, interp);
        }
        self.literal();
    }

    fn single_quoted(&mut self) -> bool {
        match self.lang {
            Lang::Jsx | Lang::Ts | Lang::Php => {
                self.i += 1;
                self.body(b"'", true, Interp::None);
                self.literal();
                true
            }
            Lang::Swift => false,
            _ => self.char_literal(),
        }
    }

    /// `'x'` or `'\n'`; anything else (a Rust lifetime, a Scala symbol, a C++
    /// digit separator) leaves the quote to be read as punctuation.
    fn char_literal(&mut self) -> bool {
        let len = if self.at(1) == b'\\' {
            (3..14)
                .find(|&j| self.at(j) == b'\'' || self.at(j) == b'\n' || self.at(j) == 0)
                .filter(|&j| self.at(j) == b'\'')
        } else {
            let b0 = self.at(1);
            let w = match b0 {
                0 | b'\n' | b'\'' => return false,
                0xF0..=0xFF => 4,
                0xE0..=0xEF => 3,
                0x80..=0xDF => 2,
                _ => 1,
            };
            (self.at(1 + w) == b'\'').then_some(1 + w)
        };
        match len {
            Some(j) => {
                self.i += j + 1;
                self.literal();
                true
            }
            None => false,
        }
    }

    fn backtick(&mut self) -> bool {
        if self.at(0) != b'`' {
            return false;
        }
        let interp = match self.lang {
            Lang::Jsx | Lang::Ts => Interp::Dollar,
            Lang::Go | Lang::Kotlin | Lang::Scala | Lang::Swift => Interp::None,
            _ => return false,
        };
        self.i += 1;
        let escapes = interp == Interp::Dollar;
        self.body(b"`", escapes, interp);
        self.literal();
        true
    }

    fn regex(&mut self) -> bool {
        if !matches!(self.lang, Lang::Jsx | Lang::Ts)
            || self.at(0) != b'/'
            || matches!(self.at(1), b'/' | b'*')
            || !self.regex_allowed()
        {
            return false;
        }
        self.regex_body(false, Interp::None);
        true
    }

    fn regex_body(&mut self, multiline: bool, interp: Interp) {
        self.i += 1;
        let mut class = false;
        while !self.done() {
            if self.interpolation(interp) {
                continue;
            }
            let ch = self.s[self.i];
            if ch == b'\\' {
                self.i += 2;
                continue;
            }
            if ch == b'\n' && !multiline {
                break;
            }
            self.i += 1;
            match ch {
                b'[' => class = true,
                b']' => class = false,
                b'/' if !class => break,
                _ => {}
            }
        }
        self.literal();
    }

    /// Raw and verbatim forms whose prefix would otherwise read as an identifier.
    fn prefixed_string(&mut self) -> bool {
        if self.prev_is_ident() {
            return false;
        }
        match self.lang {
            Lang::Rust => self.rust_raw(),
            Lang::Cpp => self.cpp_raw(),
            Lang::CSharp => self.csharp_string(),
            Lang::Swift => self.swift_raw(),
            _ => false,
        }
    }

    fn rust_raw(&mut self) -> bool {
        let mut k = usize::from(matches!(self.at(0), b'b' | b'c'));
        if self.at(k) != b'r' {
            if k == 1 && self.at(1) == b'"' {
                self.i += 1;
                self.double_quoted();
                return true;
            }
            if k == 1 && self.at(1) == b'\'' {
                self.i += 1;
                return self.char_literal();
            }
            return false;
        }
        k += 1;
        let hashes = self.run_of(k, b'#');
        if self.at(k + hashes) != b'"' {
            return false;
        }
        self.i += k + hashes + 1;
        let mut end = vec![b'"'];
        end.resize(1 + hashes, b'#');
        self.body(&end, false, Interp::None);
        self.literal();
        true
    }

    fn cpp_raw(&mut self) -> bool {
        let Some(p) = CPP_RAW_PREFIXES.into_iter().find(|p| self.starts(p)) else {
            return false;
        };
        self.i += p.len();
        let delim_start = self.i;
        while !self.done() && self.s[self.i] != b'(' && self.s[self.i] != b'\n' {
            self.i += 1;
        }
        let mut end = vec![b')'];
        end.extend_from_slice(&self.s[delim_start..self.i]);
        end.push(b'"');
        self.body(&end, false, Interp::None);
        self.literal();
        true
    }

    fn csharp_string(&mut self) -> bool {
        let dollars = self.run_of(0, b'$');
        let at_sign =
            self.at(dollars) == b'@' || (dollars == 0 && self.at(0) == b'@' && self.at(1) == b'$');
        if at_sign {
            let skip = if dollars == 0 && self.at(1) == b'$' {
                2
            } else {
                dollars + 1
            };
            if self.at(skip) != b'"' {
                return false;
            }
            let interp = if skip > 1 {
                Interp::Brace
            } else {
                Interp::None
            };
            self.i += skip + 1;
            while !self.done() {
                if self.starts(b"\"\"") {
                    self.i += 2;
                } else if self.s[self.i] == b'"' {
                    self.i += 1;
                    break;
                } else if !self.interpolation(interp) {
                    self.i += 1;
                }
            }
            self.literal();
            return true;
        }
        if self.s[self.i + dollars..].starts_with(b"\"\"\"") {
            self.i += dollars;
            let quotes = self.run_of(0, b'"');
            self.i += quotes;
            self.body(&vec![b'"'; quotes], false, Interp::None);
            self.literal();
            return true;
        }
        if dollars == 1 && self.at(1) == b'"' {
            self.i += 2;
            self.body(b"\"", true, Interp::Brace);
            self.literal();
            return true;
        }
        false
    }

    fn swift_raw(&mut self) -> bool {
        let hashes = self.run_of(0, b'#');
        if hashes == 0 || self.at(hashes) != b'"' {
            return false;
        }
        let quote: &[u8] = if self.s[self.i + hashes..].starts_with(b"\"\"\"") {
            b"\"\"\""
        } else {
            b"\""
        };
        self.i += hashes + quote.len();
        let mut end = quote.to_vec();
        end.resize(quote.len() + hashes, b'#');
        self.body(&end, false, Interp::None);
        self.literal();
        true
    }

    // -------------------------------------------------------------- JSX

    /// A JSX element in expression position: its text children are prose, not
    /// code, so an apostrophe in `<p>Don't</p>` must not open a string.
    fn jsx(&mut self) -> bool {
        if self.lang != Lang::Jsx
            || self.at(0) != b'<'
            || !self.regex_allowed()
            || matches!(self.last, Some(b'<' | b'>'))
        {
            return false;
        }
        let next = self.at(1);
        if next != b'>' && !(next.is_ascii_alphabetic() || next == b'_') {
            return false;
        }
        if next != b'>' && self.generic_params_ahead() {
            return false;
        }
        self.jsx_element();
        self.literal();
        true
    }

    /// `<T,>(…) =>` and `<T extends U>(…) =>` are TSX generics, not elements.
    fn generic_params_ahead(&self) -> bool {
        let mut j = self.i + 1;
        while j < self.s.len() && is_ident(self.s[j]) {
            j += 1;
        }
        while j < self.s.len() && self.s[j] == b' ' {
            j += 1;
        }
        self.s[j..].starts_with(b",") || self.s[j..].starts_with(b"extends ")
    }

    fn jsx_element(&mut self) {
        self.i += 1;
        if self.at(0) == b'>' {
            self.i += 1;
            self.jsx_children();
            return;
        }
        while !self.done() {
            if self.comment() {
                continue;
            }
            match self.s[self.i] {
                b'/' if self.at(1) == b'>' => {
                    self.i += 2;
                    return;
                }
                b'>' => {
                    self.i += 1;
                    self.jsx_children();
                    return;
                }
                b'{' => {
                    self.i += 1;
                    self.last = None;
                    self.code(Some(b'}'));
                }
                q @ (b'"' | b'\'') => {
                    self.i += 1;
                    self.body(&[q], false, Interp::None);
                }
                _ => self.i += 1,
            }
        }
    }

    fn jsx_children(&mut self) {
        while !self.done() {
            if self.starts(b"</") {
                while !self.done() && self.s[self.i] != b'>' {
                    self.i += 1;
                }
                self.i += 1;
                return;
            }
            match self.s[self.i] {
                b'<' => self.jsx_element(),
                b'{' => {
                    self.i += 1;
                    self.last = None;
                    self.code(Some(b'}'));
                }
                _ => self.i += 1,
            }
        }
    }

    // ------------------------------------------------------------ Python

    fn python_string(&mut self) -> bool {
        let start = self.i;
        let mut k = 0;
        if !self.prev_is_ident() {
            while k < 2 && b"rRbBfFuUtT".contains(&self.at(k)) {
                k += 1;
            }
        }
        let q = self.at(k);
        if q != b'"' && q != b'\'' {
            return false;
        }
        let prefix = &self.s[start..start + k];
        let interp = if prefix
            .iter()
            .any(|c| matches!(c, b'f' | b'F' | b't' | b'T'))
        {
            Interp::Brace
        } else {
            Interp::None
        };
        let doc = self.depth == 0
            && self.only_ws_before_on_line(start)
            && !matches!(self.last, Some(b'\\' | b'=' | b'+' | b','));
        self.i += k;
        if self.at(1) == q && self.at(2) == q {
            self.i += 3;
            self.body(&[q, q, q], true, interp);
        } else {
            self.i += 1;
            self.body(&[q], true, interp);
        }
        if doc && self.statement_ends_here() {
            self.push(start, Kind::DocString);
        }
        self.literal();
        true
    }

    fn statement_ends_here(&self) -> bool {
        let rest = &self.s[self.i.min(self.s.len())..];
        let rest = &rest[rest
            .iter()
            .take_while(|&&c| c == b' ' || c == b'\t')
            .count()..];
        rest.is_empty() || matches!(rest[0], b'\n' | b'\r' | b'#' | b';')
    }

    // -------------------------------------------------------------- Ruby

    fn ruby_literal(&mut self) -> bool {
        if self.i > 0 && self.s[self.i - 1] == b'$' {
            return false;
        }
        let c = self.at(0);
        match c {
            b'"' | b'`' => {
                self.i += 1;
                self.body(&[c], true, Interp::Hash);
                self.literal();
                true
            }
            b'\'' => {
                self.i += 1;
                self.body(b"'", true, Interp::None);
                self.literal();
                true
            }
            b'/' if self.ruby_literal_allowed() => {
                self.regex_body(true, Interp::Hash);
                true
            }
            b'?' => self.ruby_char(),
            b'%' => self.ruby_percent(),
            b'<' => self.ruby_heredoc(),
            _ => false,
        }
    }

    /// Where Ruby reads `/` or `%` as opening a literal rather than as division
    /// or modulo: in operand position, at the start of a line, or after a word
    /// with a space before the operator and none after, as in `eval %[…]`.
    fn ruby_literal_allowed(&self) -> bool {
        if self.regex_allowed() || self.only_ws_before_on_line(self.i) {
            return true;
        }
        let spaced_before = self.i > 0 && matches!(self.s[self.i - 1], b' ' | b'\t');
        let tight_after = !matches!(self.at(1), b' ' | b'\t' | b'\n' | b'=');
        self.last == Some(b'a') && spaced_before && tight_after
    }

    /// `?x` character literals, as in `?#` or `?'`; `a ? b : c` has a space.
    fn ruby_char(&mut self) -> bool {
        let next = self.at(1);
        if self.prev_is_ident()
            || matches!(self.last, Some(b')' | b']' | b'0' | b'"'))
            || !next.is_ascii_graphic()
            || is_ident(self.at(2))
        {
            return false;
        }
        self.i += 2;
        self.literal();
        true
    }

    fn ruby_percent(&mut self) -> bool {
        if !self.ruby_literal_allowed() {
            return false;
        }
        let k = if b"qQwWiIrsx".contains(&self.at(1)) {
            2
        } else {
            1
        };
        let open = self.at(k);
        if open == 0 || open.is_ascii_alphanumeric() || open.is_ascii_whitespace() {
            return false;
        }
        let close = match open {
            b'(' => b')',
            b'[' => b']',
            b'{' => b'}',
            b'<' => b'>',
            o => o,
        };
        self.i += k + 1;
        let mut nest = 0;
        while !self.done() {
            let ch = self.s[self.i];
            self.i += 1;
            if ch == b'\\' {
                self.i += 1;
            } else if ch == close && nest == 0 {
                break;
            } else if ch == close {
                nest -= 1;
            } else if ch == open && open != close {
                nest += 1;
            }
        }
        self.literal();
        true
    }

    fn ruby_heredoc(&mut self) -> bool {
        if !self.starts(b"<<") {
            return false;
        }
        let mut k = 2;
        let squiggly = matches!(self.at(k), b'~' | b'-');
        if squiggly {
            k += 1;
        }
        let quoted = matches!(self.at(k), b'"' | b'\'' | b'`');
        if quoted {
            k += 1;
        }
        let id_start = self.i + k;
        let id_len = self.s[id_start..]
            .iter()
            .take_while(|&&c| is_ident(c))
            .count();
        if id_len == 0 || !(quoted || squiggly || self.s[id_start].is_ascii_uppercase()) {
            return false;
        }
        self.heredocs
            .push(self.s[id_start..id_start + id_len].to_vec());
        self.i = id_start + id_len + usize::from(quoted);
        self.literal();
        true
    }

    /// At the newline ending a line that opened heredocs: skip their bodies.
    fn ruby_heredoc_bodies(&mut self) {
        for id in std::mem::take(&mut self.heredocs) {
            while !self.done() {
                self.i += 1;
                let line_start = self.i;
                while !self.done() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
                if self.s[line_start..self.i].trim_ascii() == id.as_slice() {
                    break;
                }
            }
        }
    }

    // --------------------------------------------------------------- PHP

    fn php_html(&mut self) {
        while !self.done() {
            if self.starts(b"<?php") {
                self.i += 5;
                return;
            }
            if self.starts(b"<?=") {
                self.i += 3;
                return;
            }
            self.i += 1;
        }
    }

    fn php_heredoc(&mut self) -> bool {
        if self.lang != Lang::Php || !self.starts(b"<<<") {
            return false;
        }
        let mut k = 3;
        while self.at(k) == b' ' {
            k += 1;
        }
        if matches!(self.at(k), b'"' | b'\'') {
            k += 1;
        }
        let id_start = self.i + k;
        let id_len = self.s[id_start..]
            .iter()
            .take_while(|&&c| is_ident(c))
            .count();
        if id_len == 0 {
            return false;
        }
        let id = self.s[id_start..id_start + id_len].to_vec();
        self.i = id_start + id_len;
        self.skip_to_eol();
        while !self.done() {
            self.i += 1;
            let line = &self.s[self.i.min(self.s.len())..];
            let indent = line
                .iter()
                .take_while(|&&c| c == b' ' || c == b'\t')
                .count();
            let rest = &line[indent..];
            if rest.starts_with(&id) && !rest.get(id.len()).is_some_and(|&c| is_ident(c)) {
                self.i += indent + id.len();
                break;
            }
            while !self.done() && self.s[self.i] != b'\n' {
                self.i += 1;
            }
        }
        self.literal();
        true
    }

    // ----------------------------------------------------------- Generic

    /// Unknown languages: quotes count as strings only when they close on
    /// the same line, so a stray apostrophe can't hide the rest of the file.
    fn generic_string(&mut self) -> bool {
        let q = self.at(0);
        if q != b'"' && q != b'\'' {
            return false;
        }
        let mut j = self.i + 1;
        while j < self.s.len() && self.s[j] != q && self.s[j] != b'\n' {
            j += if self.s[j] == b'\\' { 2 } else { 1 };
        }
        if j >= self.s.len() || self.s[j] != q {
            return false;
        }
        self.i = j + 1;
        self.literal();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(src: &str, path: &str) -> Vec<String> {
        comments(src, path)
            .into_iter()
            .map(|c| src[c.start..c.end].to_string())
            .collect()
    }

    #[test]
    fn rust_strings_lifetimes_and_nested_blocks() {
        let src = "fn f<'a>(x: &'a str) -> char {\n    let u = \"https://x.io // no\";\n    let r = r#\"/* no \"# + c'q';\n    /* outer /* inner */ still */\n    'x' // yes\n}\n";
        assert_eq!(
            found(src, "a.rs"),
            ["/* outer /* inner */ still */", "// yes"]
        );
    }

    #[test]
    fn c_and_cpp_raw_strings_and_digit_separators() {
        let src =
            "int n = 1'000'000; // one\nauto s = R\"x(// not )x\";\nchar q = '\\''; /* two */\n";
        assert_eq!(found(src, "a.cpp"), ["// one", "/* two */"]);
    }

    #[test]
    fn go_raw_and_runes() {
        let src = "s := `// no`\nr := '\"' // yes\n";
        assert_eq!(found(src, "a.go"), ["// yes"]);
    }

    #[test]
    fn js_regex_template_and_division() {
        let src = "const re = /\\/\\/ [/*]/g; // a\nconst t = `${x /* b */} // no`;\nconst d = a / b / c; // c\n";
        assert_eq!(found(src, "a.ts"), ["// a", "/* b */", "// c"]);
    }

    #[test]
    fn jsx_text_apostrophes_and_comment_children() {
        let src = "const E = () => (\n  <p className=\"x\">Don't {/* kept */} it's</p>\n);\n// after\nconst g = <T,>(x: T) => x; // generic\n";
        assert_eq!(
            found(src, "a.tsx"),
            ["/* kept */", "// after", "// generic"]
        );
    }

    #[test]
    fn python_hash_in_strings_fstrings_and_docstrings() {
        let src = "\"\"\"Module doc.\"\"\"\nurl = 'https://x.io/#frag'  # real\ns = f\"{d['#']} # no\"\ncall(\n    \"arg\"\n)\ndef f():\n    '''Doc.'''\n    return 1\n";
        assert_eq!(
            found(src, "a.py"),
            ["\"\"\"Module doc.\"\"\"", "# real", "'''Doc.'''"]
        );
        let kinds: Vec<Kind> = comments(src, "a.py").iter().map(|c| c.kind).collect();
        assert_eq!(kinds, [Kind::DocString, Kind::Line, Kind::DocString]);
    }

    #[test]
    fn fstring_format_spec_hash_is_not_a_comment() {
        let src = "s = f\"{id(x):#x}\"  # real\ndef f():\n    \"\"\"Doc.\"\"\"\n";
        assert_eq!(found(src, "a.py"), ["# real", "\"\"\"Doc.\"\"\""]);
    }

    #[test]
    fn jsx_attribute_comments_and_shift_operator() {
        let src = "const e = (\n  <Example\n    // note\n    name=\"x\"\n  />\n);\na[i] = e += 1 << r[i - 1]; // shift\n";
        assert_eq!(found(src, "a.jsx"), ["// note", "// shift"]);
    }

    #[test]
    fn ruby_interpolation_globals_heredocs_and_regex() {
        let src = "puts \"hi #{name} # no\"\nx = $' # one\nt = <<~EOS\n  # body, not a comment\nEOS\nre = /a # no\n  b/x # two\nc = ?# # three\n=begin\nblock\n=end\n";
        assert_eq!(
            found(src, "a.rb"),
            ["# one", "# two", "# three", "=begin\nblock\n=end"]
        );
    }

    #[test]
    fn ruby_literals_at_line_start_after_words_and_lowercase_heredocs() {
        let src = "v = x.gsub('a', 'b')\n%Q^#@n=\"#{v}\"^\neval %[\n  # in literal\n]\nwarn <<-eowarn\n  # in heredoc\neowarn\nm = a % b # one\nd = a / b # two\n";
        assert_eq!(found(src, "a.rb"), ["# one", "# two"]);
    }

    #[test]
    fn php_hash_attributes_html_and_heredoc() {
        let src = "<a href=\"//x\"># not php</a>\n<?php\n#[Attr]\n$x = 1; # one\n$h = <<<EOT\n// body\nEOT;\n// two ?> <b>// html</b>\n";
        assert_eq!(found(src, "a.php"), ["# one", "// two "]);
    }

    #[test]
    fn csharp_verbatim_and_interpolated() {
        let src = "var a = @\"C:\\// no\"; // one\nvar b = $\"{x /* two */} // no\";\n";
        assert_eq!(found(src, "a.cs"), ["// one", "/* two */"]);
    }

    #[test]
    fn kotlin_swift_scala_templates() {
        assert_eq!(
            found("val s = \"${m[\"k\"]} // no\" // yes\n", "a.kt"),
            ["// yes"]
        );
        assert_eq!(
            found("let s = \"\\(f(\"x\")) // no\" // yes\n", "a.swift"),
            ["// yes"]
        );
        assert_eq!(
            found("val s = s\"${a} // no\" // yes\n", "a.scala"),
            ["// yes"]
        );
    }

    #[test]
    fn java_text_block() {
        let src = "String s = \"\"\"\n  // no\n  \"\"\"; // yes\n";
        assert_eq!(found(src, "A.java"), ["// yes"]);
    }

    #[test]
    fn generic_fallback_handles_common_markers() {
        let src = "x = \"# no\" # one\ny // two\n-- three\n";
        assert_eq!(found(src, "a.zig"), ["# one", "// two", "-- three"]);
    }

    #[test]
    fn unterminated_input_does_not_panic() {
        for path in [
            "a.rs", "a.py", "a.rb", "a.php", "a.tsx", "a.cs", "a.swift", "a.cpp",
        ] {
            for src in [
                "\"", "'", "/*", "r#\"", "<<~EOS\n", "`${", "<a", "$@\"", "#\"", "f'{",
            ] {
                let _ = comments(src, path);
            }
        }
    }
}
