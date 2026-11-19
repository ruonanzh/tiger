use std::iter::Peekable;
use std::mem::take;
use std::str::Chars;

use crate::data::localization::{Language, LocaEntry, LocaValue, MacroValue};
use crate::datatype::{Code, CodeArg, CodeChain};
use crate::fileset::FileEntry;
use crate::game::Game;
use crate::parse::cob::Cob;
use crate::parse::ignore::{IgnoreFilter, IgnoreSize, parse_comment};
use crate::report::register_ignore_filter;
use crate::report::{ErrorKey, tips, untidy, warn};
use crate::token::{Loc, Token};

fn is_key_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '\''
}

// code_char might end up being identical to key_char, since we can write [gameconcept] and
// game concepts can be any id
fn is_code_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

#[derive(Clone, Debug)]
struct LocaParser {
    loc: Loc,
    offset: usize,
    content: &'static str,
    chars: Peekable<Chars<'static>>,
    language: Language,
    directory_language: Option<Language>,
    expecting_language: bool,
    language_confirmed: bool,
    header_matches_directory: bool,
    loca_end: usize,
    value: Vec<LocaValue>,
    pending_line_ignores: Vec<IgnoreFilter>,
    active_range_ignores: Vec<(u32, IgnoreFilter)>,
}

impl LocaParser {
    fn new(
        entry: &FileEntry,
        content: &'static str,
        lang: Language,
        directory_language: Option<Language>,
    ) -> Self {
        let mut chars = content.chars().peekable();
        let mut offset = 0;

        let mut loc = Loc::from(entry);
        // loc.line == 0 making this a whole file report
        loc.column = 1; // From our perspective the BOM is a character and needs to be included in column offset

        if chars.peek() == Some(&'\u{feff}') {
            offset += '\u{feff}'.len_utf8();
            loc.column += 1;
            chars.next();

            if chars.peek() == Some(&'\u{feff}') {
                // Second BOM is file content, not header, and should be reported with line number
                loc.line = 1;
                let msg = "double BOM (U+FEFF) in localization file";
                let info = "This will make the game engine skip the whole file.";
                warn(ErrorKey::Encoding).strong().msg(msg).info(info).loc(loc).push();
                offset += '\u{feff}'.len_utf8();
                loc.column += 1;
                chars.next();
            }
        } else {
            crate::report::report(
                ErrorKey::Encoding,
                if Game::is_vic3() {
                    crate::report::Severity::Tips
                } else {
                    crate::report::Severity::Warning
                },
            )
            .msg("Expected UTF-8 BOM encoding")
            .abbreviated(loc)
            .push();
        }

        // From here on we are reporting on file content
        loc.line = 1;

        LocaParser {
            loc,
            offset,
            content,
            chars,
            language: lang,
            directory_language,
            expecting_language: true,
            language_confirmed: false,
            header_matches_directory: false,
            value: Vec::new(),
            loca_end: 0,
            pending_line_ignores: Vec::new(),
            active_range_ignores: Vec::new(),
        }
    }

    fn next_char(&mut self) {
        // self.loc is always the loc of the peekable char
        if let Some(c) = self.chars.next() {
            self.offset += c.len_utf8();
            if c == '\n' {
                self.loc.line += 1;
                self.loc.column = 1;
            } else {
                self.loc.column += 1;
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.chars.peek() {
            if c.is_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    fn skip_linear_whitespace(&mut self) {
        while let Some(c) = self.chars.peek() {
            if c.is_whitespace() && *c != '\n' {
                self.next_char();
            } else {
                break;
            }
        }
    }

    fn skip_line(&mut self) {
        while let Some(&c) = self.chars.peek() {
            if c == '\n' {
                break;
            }
            self.next_char();
        }
        self.next_char(); // Eat the newline
    }

    // This function returns an Option so that the caller can return
    // its value without further boilerplate.
    #[allow(clippy::unnecessary_wraps)]
    fn error_line(&mut self, key: Token) -> Option<LocaEntry> {
        self.skip_line();
        Some(LocaEntry::new(key, LocaValue::Error, None, false))
    }

    fn get_key(&mut self) -> Token {
        let loc = self.loc;
        let start_offset = self.offset;
        while let Some(c) = self.chars.peek() {
            if is_key_char(*c) {
                self.next_char();
            } else {
                break;
            }
        }
        let s = &self.content[start_offset..self.offset];
        Token::from_static_str(s, loc)
    }

    fn unexpected_char(&mut self, expected: &str) {
        match self.chars.peek() {
            None => warn(ErrorKey::Localization)
                .msg(format!("Unexpected end of file, {expected}"))
                .loc(self.loc)
                .push(),
            Some(c) => warn(ErrorKey::Localization)
                .msg(format!("Unexpected character `{c}`, {expected}"))
                .loc(self.loc)
                .push(),
        }
    }

    // Look ahead to the last `"` on the line
    fn find_dquote(&self) -> Option<usize> {
        let mut offset = self.offset;
        let mut dquote_offset = None;
        for c in self.chars.clone() {
            if c == '"' {
                dquote_offset = Some(offset);
            } else if c == '\n' {
                return dquote_offset;
            }
            offset += c.len_utf8();
        }
        dquote_offset
    }

    fn parse_format(&mut self) -> Option<Token> {
        (self.chars.peek() == Some(&'|')).then(|| {
            self.next_char(); // eat the |
            let loc = self.loc;
            let mut text = String::new();
            while let Some(&c) = self.chars.peek() {
                if c == '$'
                    || c == ']'
                    || c == '\n'
                    || (Game::is_vic3() && self.offset >= self.loca_end)
                {
                    break;
                }
                text.push(c);
                self.next_char();
            }
            Token::new(&text, loc)
        })
    }

    fn line_has_macros(&self) -> bool {
        for c in self.chars.clone() {
            if c == '\n' {
                return false;
            } else if c == '$' {
                return true;
            }
        }
        false
    }

    fn is_currency_dollar(&self) -> bool {
        let rest = &self.content[self.offset + 1..self.loca_end];
        let after_key = rest.trim_start_matches(is_key_char);
        if after_key.starts_with(['$', '|']) {
            return false;
        }

        if rest.len() == 1 && ".,;:!?".contains(rest) {
            // A standalone currency symbol can end a prose sentence.
            return true;
        }

        // A numeric amount can follow the symbol (including after a space) or
        // precede it. Prefer a possible closing delimiter over currency, so an
        // invalid but clearly delimited substitution still gets diagnosed.
        let amount = rest.trim_start();
        let after_digits = amount.trim_start_matches(|c: char| c.is_ascii_digit());
        let numeric_prefix = amount.len() != after_digits.len()
            && !after_digits.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
        if numeric_prefix {
            let mut after_amount = after_digits;
            while let Some(group) = after_amount.strip_prefix([',', '.']) {
                let rest = group.trim_start_matches(|c: char| c.is_ascii_digit());
                if group.len() == rest.len() {
                    break;
                }
                after_amount = rest;
            }
            if let Some(unit) = after_amount.strip_prefix('/')
                && unit.starts_with(char::is_alphabetic)
            {
                let boundary = unit.trim_start_matches(char::is_alphanumeric);
                if boundary.is_empty()
                    || boundary.starts_with(|c: char| c.is_whitespace() || ".,;:!?)]".contains(c))
                {
                    // A unit rate such as $248/m3 has a complete amount and unit.
                    // A later suffix dollar or substitution belongs to a separate token.
                    return true;
                }
            }
        }
        let numeric_suffix =
            self.content[..self.offset].trim_end().ends_with(|c: char| c.is_ascii_digit())
                && (rest.is_empty()
                    || rest.starts_with(|c: char| c.is_whitespace() || "/.,;:!?)]".contains(c)));
        if !numeric_prefix && !numeric_suffix {
            return false;
        }

        !rest.split_once('$').is_some_and(|(_, tail)| {
            tail.is_empty()
                || (!tail.starts_with(is_key_char)
                    && !tail.starts_with('/')
                    && !tail.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        })
    }

    fn parse_macros(&mut self) {
        let mut v = Vec::new();
        let mut loc = self.loc;
        let mut offset = self.offset;
        let mut code_end = None;
        while let Some(&c) = self.chars.peek() {
            if c == '$' {
                // Vic3 accepts a code expression where a localization key would go.
                // Preserve the outer pair for ValueParser and still expand $keys$ in arguments.
                if code_end == Some(self.offset) {
                    code_end = None;
                    self.next_char();
                    continue;
                }
                if Game::is_vic3()
                    && code_end.is_none()
                    && let Some(value) = self.content.get(self.offset..self.loca_end)
                    && let Some(inner) = value.strip_prefix("$[")
                    && let Some((code, rest)) = inner.split_once(']')
                    && rest.starts_with('$')
                {
                    code_end = Some(self.offset + code.len() + "$[]".len());
                    self.next_char();
                    continue;
                }
                if Game::is_vic3() && self.is_currency_dollar() {
                    self.next_char();
                    continue;
                }
                let s = &self.content[offset..self.offset];
                v.push(MacroValue::Text(Token::from_static_str(s, loc)));

                let dollar = self.offset;
                let dollar_loc = self.loc;
                if let Some(mv) = self.parse_keyword() {
                    v.push(mv);
                } else if Game::is_vic3() {
                    // Keep malformed substitutions as text so later independent expressions
                    // in the same localization value are still validated.
                    let raw = &self.content[dollar..self.offset];
                    v.push(MacroValue::Text(Token::from_static_str(raw, dollar_loc)));
                } else {
                    self.value.push(LocaValue::Error);
                    return;
                }
                loc = self.loc;
                offset = self.offset;
            } else if c == '"' && self.offset == self.loca_end {
                let s = &self.content[offset..self.offset];
                let text = Token::from_static_str(s, loc);
                if Game::is_vic3() && v.is_empty() {
                    self.value = ValueParser::new_localization(vec![(&text, false)]).parse_vec();
                } else {
                    v.push(MacroValue::Text(text));
                    self.value.push(LocaValue::Macro(v));
                }
                self.next_char();
                return;
            } else {
                self.next_char();
            }
        }
        let s = &self.content[offset..self.offset];
        v.push(MacroValue::Text(Token::from_static_str(s, loc)));
        self.value.push(LocaValue::Macro(v));
    }

    fn parse_keyword(&mut self) -> Option<MacroValue> {
        let dollar = self.offset;
        self.next_char(); // Skip the $
        let loc = self.loc;
        let start_offset = self.offset;
        let key = self.get_key();
        let end_offset = self.offset;
        let format = self.parse_format();
        if self.chars.peek() != Some(&'$')
            || (Game::is_vic3() && (key.as_str().is_empty() || self.offset >= self.loca_end))
        {
            if Game::is_vic3() {
                self.report_invalid_substitution(dollar, &key);
            } else {
                let msg = "didn't recognize a key between $";
                warn(ErrorKey::Localization).weak().msg(msg).loc(key).push();
            }
            return None;
        }
        self.next_char();
        let s = &self.content[start_offset..end_offset];
        let quoted = Game::is_vic3() && format.is_some_and(|format| format.is("q"));
        Some(MacroValue::Keyword(Token::from_static_str(s, loc), quoted))
    }

    fn report_invalid_substitution(&mut self, dollar: usize, key: &Token) {
        let remainder = &self.content[self.offset..self.loca_end];
        let before = &self.content[..dollar];
        let apparent_close = remainder.split_once('$').filter(|(gap, after)| {
            let after_key = after.trim_start_matches(is_key_char);
            // A later substitution with its own key must stay available for parsing.
            let opens_next = after_key.len() < after.len() && after_key.starts_with(['$', '|']);
            !opens_next && !gap.contains(['[', ']', '"', '\n'])
        });

        let ambiguous_text = apparent_close.is_some_and(|(gap, after)| {
            let body = &self.content[dollar + 1..self.offset + gap.len()];
            !before.trim_end().ends_with(['"', '\''])
                && !after.starts_with('\'')
                && body.trim().contains(' ')
                && body.chars().all(|c| c.is_alphabetic() || c.is_whitespace() || c == '.')
        });
        // The explicit |q flag identifies intended display text even when the body
        // fails the key lexer. Keep it distinct from compact malformed substitutions.
        let quoted_display_text = apparent_close.is_some_and(|(gap, after)| {
            let body = &self.content[dollar + 1..self.offset + gap.len()];
            before.ends_with('\'')
                && after.starts_with('\'')
                && body.strip_suffix("|q").is_some_and(|phrase| {
                    phrase.trim().contains(' ')
                        && phrase.chars().all(|c| c.is_alphabetic() || c.is_whitespace())
                })
        });
        let key_started = key.as_str().starts_with(|c: char| c.is_alphanumeric() || c == '_');
        let code_started = self.content[dollar + 1..self.loca_end].starts_with('[');
        let trailing_key = before.chars().rev().take_while(|c| is_key_char(*c)).any(|c| c == '_');
        let definite = !ambiguous_text
            && !quoted_display_text
            && (key_started || code_started || trailing_key || apparent_close.is_some());

        let msg = "didn't recognize a key between $";
        if definite {
            warn(ErrorKey::Localization)
                .msg(msg)
                .info("Use a single localization key enclosed in $ delimiters")
                .loc(key)
                .push();
        } else if quoted_display_text {
            tips(ErrorKey::Localization)
                .weak()
                .msg("Quoted substitution contains display text with spaces")
                .info("Verify the intended format. Use a localization key with |q, or place literal display text directly in the argument.")
                .loc(key)
                .push();
        } else {
            tips(ErrorKey::Localization).weak().msg(msg).loc(key).push();
        }

        if let Some((gap, _)) = apparent_close {
            let end = self.offset + gap.len() + 1;
            while self.offset < end {
                self.next_char();
            }
        }
    }

    fn get_rest_of_line(&mut self) -> &str {
        let start_offset = self.offset;
        while let Some(&c) = self.chars.peek() {
            if c == '\n' {
                break;
            }
            self.next_char();
        }
        let end_offset = self.offset;
        self.next_char(); // Eat the newline
        &self.content[start_offset..end_offset]
    }

    fn skip_until_key(&mut self) {
        loop {
            // Skip comments and blank lines
            self.skip_whitespace();
            if self.chars.peek() == Some(&'#') {
                self.next_char();
                if let Some(spec) = parse_comment(self.get_rest_of_line()) {
                    match spec.size {
                        IgnoreSize::Line => self.pending_line_ignores.push(spec.filter),
                        IgnoreSize::Block => (),
                        IgnoreSize::File => {
                            register_ignore_filter(self.loc.pathname(), .., spec.filter);
                        }
                        IgnoreSize::Begin => {
                            self.active_range_ignores.push((self.loc.line + 1, spec.filter));
                        }
                        IgnoreSize::End => {
                            if let Some((start_line, filter)) = self.active_range_ignores.pop() {
                                let path = self.loc.pathname();
                                register_ignore_filter(path, start_line..self.loc.line, filter);
                            }
                        }
                    }
                }
                continue;
            }

            match self.chars.peek() {
                Some(&c) if is_key_char(c) => break,
                Some(_) => {
                    self.unexpected_char("expected localization key");
                    self.skip_line();
                }
                None => break,
            }
        }
    }

    /// Return the next `LocaEntry`, or None if there are no more in the file.
    fn parse_loca(&mut self) -> Option<LocaEntry> {
        // Loop until we have a key. Once we have a key, we'll definitely
        // return a LocaEntry for the current line, though it might be an Error entry.
        self.skip_until_key();
        self.chars.peek()?;
        for filter in self.pending_line_ignores.drain(..) {
            let path = self.loc.pathname();
            let line = self.loc.line;
            register_ignore_filter(path, line..=line, filter);
        }

        let key = self.get_key();
        self.skip_linear_whitespace();
        if self.chars.peek() == Some(&':') {
            self.next_char();
        } else {
            self.unexpected_char("expected `:`");
            return self.error_line(key);
        }

        // Read the optional revision number even when separated by whitespace in Vic3.
        if Game::is_vic3() {
            self.skip_linear_whitespace();
        }
        while let Some(c) = self.chars.peek() {
            if c.is_ascii_digit() {
                self.next_char();
            } else {
                break;
            }
        }
        self.skip_linear_whitespace();

        // Now we should see the value. But what if the line ends here?
        if matches!(self.chars.peek(), Some('#' | '\n') | None) {
            if self.expecting_language {
                self.language_confirmed = key.is(&format!("l_{}", self.language));
                if !self.language_confirmed {
                    if Game::is_vic3()
                        && let Some(language) = self.directory_language
                        && key.is(&format!("l_{language}"))
                    {
                        self.header_matches_directory = true;
                        warn(ErrorKey::Filename)
                            .msg(format!("Localization filename suffix disagrees with the l_{language} header and directory"))
                            .info(format!("Rename the file so it ends with _l_{language}.yml."))
                            .loc(key)
                            .push();
                    } else {
                        let msg =
                            format!("wrong language header, should be `l_{}:`", self.language);
                        warn(ErrorKey::Localization).msg(msg).loc(key).push();
                    }
                }
                self.expecting_language = false;
                self.skip_line();
                // Recursing here is safe because it can happen only once.
                return self.parse_loca();
            }
            warn(ErrorKey::Localization).msg("key with no value").loc(&key).push();
            return self.error_line(key);
        } else if self.expecting_language {
            let msg = format!("expected language header `l_{}:`", self.language);
            warn(ErrorKey::Localization).msg(msg).loc(&key).push();
            self.expecting_language = false;
            // Continue to parse this entry as usual
        }
        if self.chars.peek() == Some(&'"') {
            self.next_char();
        } else {
            self.unexpected_char("expected `\"`");
            return self.error_line(key);
        }

        // We need to pre-parse because the termination of localization entries
        // is ambiguous. A loca value ends at the last " on the line.
        // Any # or " before that are part of the value; an # after that
        // introduces a comment.
        if let Some(i) = self.find_dquote() {
            self.loca_end = i;
        } else {
            let msg = "localization entry without ending quote";
            warn(ErrorKey::Localization).msg(msg).loc(self.loc).push();
            return self.error_line(key);
        }

        self.value = Vec::new();
        let s = &self.content[self.offset..self.loca_end];
        let token = Token::from_static_str(s, self.loc);

        // We also need to pre-parse because $macros$ can appear anywhere and
        // we don't know how to parse the results until we know what to
        // substitute. If there are macros in the line, return it as a special
        // `LocaValue::Macro` array
        if Game::is_vic3() && s == "[" {
            // The base game localizes an opening bracket as its own key.
            self.value.push(LocaValue::Text(token.clone()));
            while self.offset <= self.loca_end {
                self.next_char();
            }
        } else if self.line_has_macros() {
            self.parse_macros();
            if matches!(self.value.last(), Some(&LocaValue::Error)) {
                return self.error_line(key);
            }
        } else {
            self.value = ValueParser::new_localization(vec![(&token, false)]).parse_vec();
            while self.offset <= self.loca_end {
                self.next_char();
            }
        }

        self.skip_linear_whitespace();
        let clean_terminator = matches!(self.chars.peek(), None | Some('#' | '\n'));
        if !clean_terminator {
            let msg = "content after final `\"` on line";
            warn(ErrorKey::Localization).strong().msg(msg).loc(self.loc).push();
        }

        self.skip_line();
        let value = if self.value.len() == 1 {
            self.value.remove(0)
        } else {
            LocaValue::Concat(take(&mut self.value))
        };
        let source_value_unambiguous =
            Game::is_vic3() && clean_terminator && has_only_escaped_quotes(s);
        Some(LocaEntry::new(key, value, Some(token), source_value_unambiguous))
    }
}

// The parser's last-quote recovery is permissive. Keep recovering these values, but don't
// use a value with an ambiguous quote boundary as proof of a localization conflict.
fn has_only_escaped_quotes(value: &str) -> bool {
    let mut escaped = false;
    for byte in value.bytes() {
        if byte == b'"' && !escaped {
            return false;
        }
        escaped = byte == b'\\' && !escaped;
    }
    !escaped
}

pub struct ValueParser<'a> {
    loc: Loc,
    offset: usize,
    content: Vec<&'a Token>,
    content_iters: Vec<Peekable<Chars<'a>>>,
    content_idx: usize,
    quoted_expansions: Vec<bool>,
    is_localization: bool,
    value: Vec<LocaValue>,
}

// TODO: some duplication of helper functions between `LocaParser` and `ValueParser`
impl<'a> ValueParser<'a> {
    pub fn new(content: Vec<&'a Token>) -> Self {
        assert!(!content.is_empty());

        Self {
            loc: content[0].loc,
            offset: 0,
            content_iters: content.iter().map(|t| t.as_str().chars().peekable()).collect(),
            content,
            content_idx: 0,
            quoted_expansions: Vec::new(),
            is_localization: false,
            value: Vec::new(),
        }
    }

    pub(crate) fn new_localization(content: Vec<(&'a Token, bool)>) -> Self {
        let (content, quoted_expansions) = content.into_iter().unzip();
        let mut parser = Self::new(content);
        parser.quoted_expansions = quoted_expansions;
        parser.is_localization = true;
        parser
    }

    fn maybe_advance_idx(&mut self) -> bool {
        if self.content_idx + 1 == self.content.len() {
            false
        } else {
            self.content_idx += 1;
            self.loc = self.content[self.content_idx].loc;
            self.offset = 0;
            true
        }
    }

    fn peek(&mut self) -> Option<char> {
        if let Some(p) = self.content_iters[self.content_idx].peek() {
            Some(*p)
        } else if self.maybe_advance_idx() {
            self.peek()
        } else {
            None
        }
    }

    fn next_char(&mut self) {
        if let Some(c) = self.content_iters[self.content_idx].next() {
            self.offset += c.len_utf8();
            self.loc.column += 1;
        } else if self.maybe_advance_idx() {
            self.next_char();
        }
    }

    fn start_text(&self) -> Cob {
        let mut cob = Cob::new();
        cob.set(self.content[self.content_idx].as_str(), self.offset, self.loc);
        cob
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    fn unexpected_char(&mut self, expected: &str, errorkey: ErrorKey) {
        // TODO: handle EOF better
        let c = self.peek().unwrap_or(' ');
        let msg = format!("Unexpected character `{c}`, {expected}");
        warn(errorkey).msg(msg).loc(self.loc).push();
    }

    fn get_key(&mut self) -> Token {
        let mut text = self.start_text();
        while let Some(c) = self.peek() {
            if is_key_char(c) {
                text.add_char(c);
                self.next_char();
            } else {
                break;
            }
        }
        text.take_to_token()
    }

    fn parse_format(&mut self) -> Option<Token> {
        (self.peek() == Some('|')).then(|| {
            self.next_char(); // eat the |
            let mut text = self.start_text();
            while let Some(c) = self.peek() {
                if c == '$' || c == ']' {
                    break;
                }
                text.add_char(c);
                self.next_char();
            }
            text.take_to_token()
        })
    }

    fn parse_code_args(&mut self) -> Vec<CodeArg> {
        self.next_char(); // eat the opening (
        let mut v = Vec::new();

        loop {
            self.skip_whitespace();
            if self.peek() == Some('\'') {
                self.next_char();
                let loc = self.loc;
                let mut text = self.start_text();
                let mut reported_expansion = None;
                let mut unreliable_literal = false;
                while let Some(c) = self.peek() {
                    if self.quoted_expansions.get(self.content_idx) == Some(&true) {
                        // A |q expansion contributes literal text to the enclosing argument.
                        text.add_char(c);
                        self.next_char();
                        continue;
                    }
                    match c {
                        '\'' => {
                            // A single quote in a macro's prose is not a delimiter authored
                            // by the caller. Recover its text and point at the substitution.
                            if Game::is_vic3()
                                && let Some(link) = self.loc.link_idx
                                && Some(link) != loc.link_idx
                                && let Some(call) = crate::macros::MACRO_MAP.get_loc(link)
                                && self.content[self.content_idx].as_str()[self.offset + 1..]
                                    .starts_with(char::is_alphabetic)
                            {
                                if reported_expansion != Some(link) {
                                    tips(ErrorKey::Datafunctions)
                                        .weak()
                                        .msg("Substitution introduces an apostrophe into a quoted argument")
                                        .info("Use |q on the substitution to keep the apostrophe as literal text.")
                                        .loc(call)
                                        .loc_msg(self.loc, "apostrophe from localization value")
                                        .push();
                                    reported_expansion = Some(link);
                                }
                            } else {
                                break;
                            }
                        }
                        ']' => {
                            unreliable_literal = Game::is_vic3();
                            let msg = "possible unterminated argument string";
                            let info = "Using [ ] inside argument strings does not work";
                            warn(ErrorKey::Localization).msg(msg).info(info).loc(self.loc).push();
                        }
                        '\u{feff}' => {
                            let msg = "found U+FEFF (UTF-8 BOM) in the middle of the file";
                            warn(ErrorKey::ParseError).strong().msg(msg).loc(self.loc).push();
                        }
                        _ => (),
                    }
                    text.add_char(c);
                    self.next_char();
                }
                if self.peek() != Some('\'') {
                    warn(ErrorKey::Localization)
                        .msg("unterminated argument string")
                        .loc(loc)
                        .push();
                    self.value.push(LocaValue::Error);
                    return Vec::new();
                }
                v.push(if unreliable_literal || reported_expansion.is_some() {
                    CodeArg::UnreliableLiteral
                } else {
                    CodeArg::Literal(text.take_to_token())
                });
                self.next_char();
            } else if self.peek() == Some(')') {
                // Empty () means no arguments
            } else {
                v.push(CodeArg::Chain(self.parse_code_inner()));
            }
            self.skip_whitespace();
            if self.peek() != Some(',') {
                break;
            }
            self.next_char(); // Eat the comma
        }
        if self.peek() == Some(')') {
            self.next_char();
        } else {
            self.unexpected_char("expected `)`", ErrorKey::Datafunctions);
        }
        v
    }

    fn parse_code_code(&mut self) -> Code {
        let mut text = self.start_text();

        if Game::is_hoi4() && self.peek() == Some('?') {
            text.add_char('?');
            self.next_char();
        }

        while let Some(c) = self.peek() {
            if is_code_char(c) {
                text.add_char(c);
                self.next_char();
            } else {
                break;
            }
        }
        let name = text.take_to_token();
        self.review_code_spacing(|c| c == '(');
        if self.peek() == Some('(') {
            Code { name, arguments: self.parse_code_args(), parenthesized: true }
        } else {
            Code { name, arguments: Vec::new(), parenthesized: false }
        }
    }

    fn review_code_spacing(&mut self, is_delimiter: impl FnOnce(char) -> bool) {
        if !Game::is_vic3() || !matches!(self.peek(), Some(' ' | '\t')) {
            return;
        }
        let tail = &self.content[self.content_idx].as_str()[self.offset..];
        if tail.trim_start_matches([' ', '\t']).chars().next().is_some_and(is_delimiter) {
            tips(ErrorKey::Datafunctions)
                .weak()
                .msg("whitespace between data function tokens needs runtime verification")
                .loc(self.loc)
                .push();
            self.skip_whitespace();
        }
    }

    fn parse_code_inner(&mut self) -> CodeChain {
        let mut v = Vec::new();
        loop {
            v.push(self.parse_code_code());
            // Newlines followed by whitespace are allowed in code sequences,
            // but not random whitespace.
            if matches!(self.peek(), Some('\r' | '\n')) {
                self.next_char();
                self.skip_whitespace();
            }
            self.review_code_spacing(|c| c == '.');
            if self.peek() != Some('.') {
                break;
            }
            self.next_char(); // Eat the '.'
            self.review_code_spacing(is_code_char);
        }
        CodeChain { codes: v.into_boxed_slice() }
    }

    fn parse_code(&mut self) {
        self.next_char(); // eat the opening [
        self.skip_whitespace();

        let chain = self.parse_code_inner();

        self.skip_whitespace();

        // The game engine doesn't mind if there are too many `)` before the `]`, so handle
        // that at "untidy" severity.
        let mut warned_extra_parens = false;
        while self.peek() == Some(')') {
            if !warned_extra_parens {
                let msg = "too many `)`";
                untidy(ErrorKey::Datafunctions).msg(msg).loc(self.loc).push();
                warned_extra_parens = true;
            }
            self.next_char();
            self.skip_whitespace();
        }

        let format = self.parse_format();

        if self.peek() == Some(']') {
            self.next_char();
            self.value.push(LocaValue::Code(chain, format));
        } else if Game::is_vic3()
            && self.peek() == Some('[')
            && let Some(link) = self.loc.link_idx
            && let Some(call) = crate::macros::MACRO_MAP.get_loc(link)
            && let Some(last_code) = chain.codes.last()
            && last_code.name.loc.link_idx != Some(link)
        {
            warn(ErrorKey::Datafunctions)
                .msg("Localization substitution starts another data function before the current chain closes")
                .info("Close the current chain before the substitution or repair the data function around it.")
                .loc(call)
                .loc_msg(&last_code.name, "unclosed data function chain")
                .loc_msg(self.loc, "expanded data function starts here")
                .push();
            self.value.push(LocaValue::Code(chain, format));
            // Leave the next chain available for independent validation.
        } else {
            self.unexpected_char("expected `]`", ErrorKey::Datafunctions);
            self.value.push(LocaValue::Error);
        }
    }

    fn handle_tooltip(&mut self, value: &str, loc: Loc) {
        if value.contains(',') {
            // If the value contains commas, then it's #tooltip:tag,key or #tooltip:tag,key,value
            // Separate out the tooltip.
            let value = Token::new(value, loc);
            let values: Vec<_> = value.split(',');
            self.value
                .push(LocaValue::ComplexTooltip(Box::new(values[0].clone()), values[1].clone()));
            return;
        }

        // Otherwise, then it's just #tooltip:tooltip
        self.value.push(LocaValue::Tooltip(Token::new(value, loc)));
    }

    fn parse_markup(&mut self) {
        let loc = self.loc;
        self.next_char(); // skip the #
        if self.peek() == Some('#') {
            // double # means a literal #
            self.next_char();
            self.value.push(LocaValue::Text(Token::from_static_str("#", loc)));
        } else if self.peek() == Some('!') {
            self.next_char();
            self.value.push(LocaValue::MarkupEnd);
        } else {
            // examples:
            // #indent_newline:2
            // #color:{1.0,1.0,1.0}
            // #font:TitleFont
            // #tooltippable;positive_value;TOOLTIP:expedition_progress_explanation_tt
            // #TOOLTIP:GAME_TRAIT,lifestyle_physician,[GetNullCharacter]
            // #tooltip:[Party.GetTooltipTag]|[InterestGroup.GetTooltipTag],INTEREST_GROUP_AFFILIATION_BREAKDOWN
            enum State {
                InKey(String),
                InValue(String, String, Loc, usize),
            }
            let mut state = State::InKey(String::new());
            let mut tooltip_value_start = None;
            let mut dynamic_value = false;
            while let Some(c) = self.peek() {
                if c.is_whitespace() {
                    break;
                }
                let mut consumed = false;
                match &mut state {
                    State::InKey(s) => {
                        if c == ':' {
                            if s.is_empty() {
                                self.unexpected_char("expected markup key", ErrorKey::Markup);
                            }
                            tooltip_value_start = Some((self.content_idx, self.offset + 1));
                            dynamic_value = false;
                            state = State::InValue(s.clone(), String::new(), self.loc, 0);
                        } else if c == ';' {
                            if s.is_empty() {
                                self.unexpected_char("expected markup key", ErrorKey::Markup);
                            }
                            // TODO: warn about markup keys that expect a value
                            state = State::InKey(String::new());
                        } else if c.is_alphanumeric() || c == '_' {
                            s.push(c);
                        } else if Game::is_vic3() && c == '[' {
                            // A data function can supply the markup key, for example a morale color.
                            s.push('[');
                            self.parse_code();
                            consumed = true;
                        } else {
                            break;
                        }
                    }
                    State::InValue(key, value, loc, bracecount) => {
                        if c == ':' {
                            let text = self.content[self.content_idx].as_str();
                            if Game::is_vic3()
                                && dynamic_value
                                && key.eq_ignore_ascii_case("tooltip")
                                && value.starts_with(',')
                                && value.len() > 1
                                && text[self.offset + 1..]
                                    .chars()
                                    .next()
                                    .is_some_and(char::is_whitespace)
                                && let Some((idx, start)) = tooltip_value_start
                                && idx == self.content_idx
                            {
                                let target = &text[start..self.offset];
                                tips(ErrorKey::Markup)
                                    .weak()
                                    .msg(format!(
                                        "dynamic tooltip target has a trailing colon: {target}"
                                    ))
                                    .loc(self.loc)
                                    .push();
                            } else {
                                value.push(c);
                                self.unexpected_char("expected `;`", ErrorKey::Markup);
                            }
                        } else if c == ';' {
                            if key.eq_ignore_ascii_case("tooltip") {
                                self.handle_tooltip(value, *loc);
                            }
                            state = State::InKey(String::new());
                        } else if c == '{' {
                            *bracecount += 1;
                        } else if c == '}' {
                            if *bracecount > 0 {
                                *bracecount -= 1;
                            } else {
                                let msg = "mismatched braces in markup";
                                warn(ErrorKey::Markup).msg(msg).loc(self.loc).push();
                                self.value.push(LocaValue::Error);
                            }
                        } else if c == '.'
                            || c == ','
                            || c.is_alphanumeric()
                            || c == '_'
                            || c == '|'
                        {
                            // . and , are freely allowed in markup values because with #tooltip
                            // the value might be a loca key.
                            // | is allowed because in some tooltips it separates components of the tooltip tag
                            value.push(c);
                        } else if c == '[' {
                            dynamic_value = true;
                            // Generating part of the markup with a code block is valid.
                            // Assume (hope) that it generates the current value and not some random chunk
                            // of markup. The next thing we see ought to be a comma or a space.
                            self.parse_code();
                            consumed = true;
                        } else {
                            break;
                        }
                    }
                }
                if !consumed {
                    self.next_char();
                }
            }
            // Leave the next opening tag for the outer parser.
            let adjacent_tag = Game::is_vic3()
                && matches!(&state, State::InKey(key) if !key.is_empty())
                && self.peek() == Some('#')
                && self.content[self.content_idx].as_str()[self.offset + 1..]
                    .starts_with(|c: char| c.is_ascii_alphabetic());
            // Clean up leftover state at end
            match state {
                State::InKey(_) => {
                    self.value.push(LocaValue::Markup);
                }
                State::InValue(key, value, loc, bracecount) => {
                    if key.eq_ignore_ascii_case("tooltip") {
                        self.handle_tooltip(&value, loc);
                    }
                    if bracecount > 0 {
                        let msg = "mismatched braces in markup";
                        warn(ErrorKey::Markup).msg(msg).loc(self.loc).push();
                        self.value.push(LocaValue::Error);
                    } else {
                        self.value.push(LocaValue::Markup);
                    }
                }
            }
            if adjacent_tag {
                return;
            }
            if self.peek().is_none_or(char::is_whitespace) {
                self.next_char();
            } else if self.content[self.content_idx].as_str()[self.offset..].starts_with("\\n") {
                // An escaped newline separates the markup key from its text.
                // Preserve the newline as text.
                self.parse_escape();
            } else {
                let msg = "#markup should be followed by a space";
                if Game::is_vic3() {
                    tips(ErrorKey::Markup).weak().msg(msg).loc(self.loc).push();
                } else {
                    warn(ErrorKey::Markup).msg(msg).loc(self.loc).push();
                    self.value.push(LocaValue::Error);
                }
            }
        }
    }

    fn parse_icon(&mut self) {
        let opening = self.loc;
        self.next_char(); // eat the @

        let mut old_value = take(&mut self.value);
        let mut terminated = false;

        while let Some(c) = self.peek() {
            if c == '[' {
                self.parse_code();
                if Game::is_vic3() && matches!(self.value.last(), Some(LocaValue::Error)) {
                    break;
                }
            } else if is_key_char(c) {
                let key = self.get_key();
                self.value.push(LocaValue::Text(key));
            } else if c == '!' {
                self.next_char();
                terminated = true;
                break;
            } else if self.value.is_empty() {
                self.unexpected_char("expected icon name", ErrorKey::Localization);
                self.value.push(LocaValue::Error);
                break;
            } else {
                self.unexpected_char("expected `!`", ErrorKey::Localization);
                self.value.push(LocaValue::Error);
                break;
            }
        }

        if Game::is_vic3() && !matches!(self.value.last(), Some(LocaValue::Error)) {
            let error = if self.value.is_empty() {
                Some("expected icon name after @")
            } else if !terminated {
                Some("expected ! after icon name")
            } else {
                None
            };
            if let Some(message) = error {
                warn(ErrorKey::Localization).msg(message).loc(opening).push();
                self.value.push(LocaValue::Error);
            }
        }

        if matches!(self.value.last(), Some(LocaValue::Error)) {
            let recovered = if Game::is_vic3() {
                // The diagnostic marks the invalid icon. Keep parsing from the boundary
                // so a typo in another expression on this line is still visible.
                LocaValue::Text(Token::new("@", opening))
            } else {
                LocaValue::Error
            };
            old_value.push(recovered);
            self.value = take(&mut old_value);
        } else if self.value.len() == 1 {
            if let Some(LocaValue::Text(icon)) = self.value.last() {
                // The usual case: a simple @icon!
                old_value.push(LocaValue::Icon(icon.clone()));
                self.value = take(&mut old_value);
            } else {
                // This can happen if the whole icon is a @[...]!
                old_value.push(LocaValue::CalculatedIcon(take(&mut self.value)));
                self.value = take(&mut old_value);
            }
        } else {
            old_value.push(LocaValue::CalculatedIcon(take(&mut self.value)));
            self.value = take(&mut old_value);
        }
    }

    #[allow(dead_code)] // only needed for hoi4
    fn parse_flag(&mut self) {
        self.next_char(); // eat the @

        let mut text = self.start_text();
        while let Some(c) = self.peek() {
            if c.is_ascii_uppercase() {
                text.add_char(c);
                self.next_char();
            } else {
                break;
            }
        }
        let flag = text.take_to_token();
        if flag.is("") {
            self.unexpected_char("expected country tag", ErrorKey::Localization);
            self.value.push(LocaValue::Error);
        } else {
            self.value.push(LocaValue::Flag(flag));
        }
    }

    fn parse_escape(&mut self) {
        let loc = self.loc;
        self.next_char(); // Skip the \
        let s = match self.peek() {
            Some('n') => '\n'.to_string(),
            Some(c) => c.to_string(),
            None => {
                self.value.push(LocaValue::Error);
                return;
            }
        };
        self.next_char();
        self.value.push(LocaValue::Text(Token::new(&s, loc)));
    }

    fn parse_text(&mut self) {
        let mut text = self.start_text();
        while let Some(c) = self.peek() {
            match c {
                '[' | '#' | '@' | '\\' => break,
                _ => {
                    text.add_char(c);
                    self.next_char();
                }
            }
        }
        self.value.push(LocaValue::Text(text.take_to_token()));
    }

    fn parse_bracketed_prose(&mut self) -> bool {
        let content = self.content[self.content_idx].as_str();
        let tail = &content[self.offset + 1..];
        let Some((inside, after)) = tail.split_once(']') else {
            return false;
        };
        if !matches!(inside, "…" | "...")
            && (inside.is_empty() || !inside.bytes().all(|c| c.is_ascii_digit()))
        {
            return false;
        }
        // Citations and editorial omissions appear within prose. Keep diagnosing
        // isolated numeric or punctuation-only expressions in localization values.
        if self.value.is_empty() && after.trim().is_empty() {
            return false;
        }
        let token = Token::new(&content[self.offset..self.offset + inside.len() + 2], self.loc);
        for _ in 0..inside.chars().count() + 2 {
            self.next_char();
        }
        self.value.push(LocaValue::Text(token));
        true
    }

    pub fn parse_vec(mut self) -> Vec<LocaValue> {
        while let Some(c) = self.peek() {
            match c {
                '[' if Game::is_vic3() && self.is_localization && self.parse_bracketed_prose() => {
                    ()
                }
                '[' => self.parse_code(),
                '#' => self.parse_markup(),
                '@' if Game::is_hoi4() => self.parse_flag(),
                '@' => self.parse_icon(),
                '\\' => self.parse_escape(),
                _ => self.parse_text(),
            }
            if matches!(self.value.last(), Some(&LocaValue::Error)) {
                return vec![LocaValue::Error];
            }
        }
        self.value
    }

    pub fn parse(self) -> LocaValue {
        let mut value = self.parse_vec();
        if value.len() == 1 { value.remove(0) } else { LocaValue::Concat(value) }
    }
}

pub struct LocaReader {
    parser: LocaParser,
}

impl LocaReader {
    pub(crate) fn language_confirmed(&self) -> bool {
        self.parser.language_confirmed
    }

    pub(crate) fn header_matches_directory(&self) -> bool {
        self.parser.header_matches_directory
    }
}

impl Iterator for LocaReader {
    type Item = LocaEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.parser.parse_loca()
    }
}

pub fn parse_loca(
    entry: &FileEntry,
    content: String,
    lang: Language,
    directory_language: Option<Language>,
) -> LocaReader {
    let content = content.leak();
    let parser = LocaParser::new(entry, content, lang, directory_language);
    LocaReader { parser }
}
