//! XML1.0 DTD well-formedness grammar; non-validating like source ElementTree.
use super::arxiv::{name_start, xml_char};
type Result<T> = std::result::Result<T, ()>;
struct Cursor<'a> {
    s: &'a str,
}
impl<'a> Cursor<'a> {
    fn new(s: &'a str) -> Self {
        Self { s }
    }
    fn space(&mut self) -> bool {
        let before = self.s.len();
        self.s = self.s.trim_start_matches([' ', '\t', '\r', '\n']);
        self.s.len() != before
    }
    fn required_space(&mut self) -> Result<()> {
        self.space().then_some(()).ok_or(())
    }
    fn eat(&mut self, s: &str) -> bool {
        if let Some(rest) = self.s.strip_prefix(s) {
            self.s = rest;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) -> Result<()> {
        self.eat(s).then_some(()).ok_or(())
    }
    fn name(&mut self, nmtoken: bool) -> Result<&'a str> {
        let mut end = 0;
        for (i, c) in self.s.char_indices() {
            let okay = if i == 0 && !nmtoken {
                name_start(c)
            } else {
                name_start(c)
                    || matches!(c,'-'|'.'|'0'..='9'|'\u{b7}'|'\u{300}'..='\u{36f}'|'\u{203f}'..='\u{2040}')
            };
            if !okay {
                break;
            }
            end = i + c.len_utf8();
        }
        if end == 0 {
            return Err(());
        }
        let name = &self.s[..end];
        self.s = &self.s[end..];
        Ok(name)
    }
    fn literal(&mut self, public: bool, attribute: bool) -> Result<&'a str> {
        let q = self.s.chars().next().ok_or(())?;
        if !matches!(q, '\'' | '"') {
            return Err(());
        }
        self.s = &self.s[1..];
        let end = self.s.find(q).ok_or(())?;
        let out = &self.s[..end];
        self.s = &self.s[end + 1..];
        if public
            && !out
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || " \r\n-'()+,./:=?;!*#@$_%".contains(c))
        {
            return Err(());
        }
        if attribute && out.contains('<') {
            return Err(());
        }
        Ok(out)
    }
    fn external(&mut self, notation: bool) -> Result<()> {
        if self.eat("SYSTEM") {
            self.required_space()?;
            self.literal(false, false)?;
        } else {
            self.expect("PUBLIC")?;
            self.required_space()?;
            self.literal(true, false)?;
            let space = self.space();
            if !notation || self.s.starts_with(['\'', '"']) {
                if !space {
                    return Err(());
                }
                self.literal(false, false)?;
            }
        }
        Ok(())
    }
    fn enum_type(&mut self, nmtoken: bool) -> Result<()> {
        self.expect("(")?;
        self.space();
        self.name(nmtoken)?;
        loop {
            self.space();
            if self.eat(")") {
                return Ok(());
            }
            self.expect("|")?;
            self.space();
            self.name(nmtoken)?;
        }
    }
}
fn references(s: &str, parameter_allowed: bool) -> Result<()> {
    let mut c = Cursor::new(s);
    while !c.s.is_empty() {
        let ch = c.s.chars().next().unwrap();
        c.s = &c.s[ch.len_utf8()..];
        if ch == '%' && !parameter_allowed {
            return Err(());
        }
        if ch == '&' {
            if c.eat("#") {
                let hex = c.eat("x");
                let end = c.s.find(';').ok_or(())?;
                let digits = &c.s[..end];
                if digits.is_empty()
                    || !digits.bytes().all(|b| {
                        if hex {
                            b.is_ascii_hexdigit()
                        } else {
                            b.is_ascii_digit()
                        }
                    })
                {
                    return Err(());
                }
                let cp = u32::from_str_radix(digits, if hex { 16 } else { 10 }).map_err(|_| ())?;
                if !char::from_u32(cp).is_some_and(xml_char) {
                    return Err(());
                }
                c.s = &c.s[end + 1..];
            } else {
                c.name(false)?;
                c.expect(";")?;
            }
        }
    }
    Ok(())
}
async fn element(c: &mut Cursor<'_>, deadline: Option<std::time::Instant>) -> Result<()> {
    c.required_space()?;
    c.name(false)?;
    c.required_space()?;
    if c.eat("EMPTY") || c.eat("ANY") {
        return Ok(());
    }
    c.expect("(")?;
    c.space();
    if c.eat("#PCDATA") {
        let mut names = false;
        loop {
            c.space();
            if c.eat(")") {
                if names {
                    c.expect("*")?;
                } else {
                    c.eat("*");
                }
                return Ok(());
            }
            c.expect("|")?;
            c.space();
            c.name(false)?;
            names = true;
        }
    }
    // Content models may nest deeply; track their grammar iteratively.
    let mut groups = vec![(None::<char>, 0usize, true)];
    let mut iterations = 0usize;
    loop {
        if iterations.is_multiple_of(128) {
            super::arxiv::checkpoint(deadline).await.map_err(|_| ())?;
        }
        iterations += 1;
        c.space();
        let g = groups.last_mut().ok_or(())?;
        if g.2 {
            if c.eat("(") {
                groups.push((None, 0, true));
                continue;
            }
            c.name(false)?;
            if !c.eat("?") && !c.eat("*") {
                c.eat("+");
            }
            g.1 += 1;
            g.2 = false;
        } else if c.eat(")") {
            let (_, count, expecting) = groups.pop().unwrap();
            if count == 0 || expecting {
                return Err(());
            }
            if !c.eat("?") && !c.eat("*") {
                c.eat("+");
            }
            if let Some(g) = groups.last_mut() {
                g.1 += 1;
                g.2 = false;
            } else {
                return Ok(());
            }
        } else {
            let sep = if c.eat(",") {
                ','
            } else {
                c.expect("|")?;
                '|'
            };
            if g.0.is_some_and(|x| x != sep) {
                return Err(());
            }
            g.0 = Some(sep);
            g.2 = true;
        }
    }
}
/// Validate declaration/DTD syntax without fetching external subsets.
pub(super) async fn validate_dtd(text: &str, deadline: Option<std::time::Instant>) -> Result<()> {
    let mut c = Cursor::new(text);
    c.space();
    c.name(false)?;
    let space = c.space();
    if c.s.starts_with("SYSTEM") || c.s.starts_with("PUBLIC") {
        if !space {
            return Err(());
        }
        c.external(false)?;
        c.space();
    }
    if c.s.is_empty() {
        return Ok(());
    }
    c.expect("[")?;
    let mut iterations = 0usize;
    loop {
        if iterations.is_multiple_of(128) {
            super::arxiv::checkpoint(deadline).await.map_err(|_| ())?;
        }
        iterations += 1;
        c.space();
        if c.eat("]") {
            c.space();
            return c.s.is_empty().then_some(()).ok_or(());
        }
        if c.eat("<!--") {
            let end = c.s.find("-->").ok_or(())?;
            let body = &c.s[..end];
            if body.contains("--") || body.ends_with('-') {
                return Err(());
            }
            c.s = &c.s[end + 3..];
            continue;
        }
        if c.eat("<?") {
            let target = c.name(false)?;
            if target.eq_ignore_ascii_case("xml") {
                return Err(());
            }
            if !c.s.starts_with("?>") {
                c.required_space()?;
            }
            let end = c.s.find("?>").ok_or(())?;
            c.s = &c.s[end + 2..];
            continue;
        }
        if c.eat("%") {
            c.name(false)?;
            c.expect(";")?;
            continue;
        }
        if c.eat("<!ELEMENT") {
            element(&mut c, deadline).await?;
        } else if c.eat("<!NOTATION") {
            c.required_space()?;
            c.name(false)?;
            c.required_space()?;
            c.external(true)?;
        } else if c.eat("<!ENTITY") {
            c.required_space()?;
            let parameter = c.eat("%");
            if parameter {
                c.required_space()?;
            }
            c.name(false)?;
            c.required_space()?;
            if c.s.starts_with(['\'', '"']) {
                let value = c.literal(false, false)?;
                references(value, false)?;
            } else {
                c.external(false)?;
                let space = c.space();
                if c.eat("NDATA") {
                    if !space || parameter {
                        return Err(());
                    }
                    c.required_space()?;
                    c.name(false)?;
                }
            }
        } else if c.eat("<!ATTLIST") {
            c.required_space()?;
            c.name(false)?;
            loop {
                let space = c.space();
                if c.s.starts_with('>') {
                    break;
                }
                if !space {
                    return Err(());
                }
                c.name(false)?;
                c.required_space()?;
                if c.s.starts_with('(') {
                    c.enum_type(true)?;
                } else {
                    let t = c.name(false)?;
                    if t == "NOTATION" {
                        c.required_space()?;
                        c.enum_type(false)?;
                    } else if !matches!(
                        t,
                        "CDATA"
                            | "ID"
                            | "IDREF"
                            | "IDREFS"
                            | "ENTITY"
                            | "ENTITIES"
                            | "NMTOKEN"
                            | "NMTOKENS"
                    ) {
                        return Err(());
                    }
                }
                c.required_space()?;
                if !c.eat("#REQUIRED") && !c.eat("#IMPLIED") {
                    if c.eat("#FIXED") {
                        c.required_space()?;
                    }
                    let value = c.literal(false, true)?;
                    references(value, true)?;
                }
            }
        } else {
            return Err(());
        }
        c.space();
        c.expect(">")?;
    }
}
/// XML declaration grammar (encoding is deliberately ignored for Unicode input).
pub(super) fn validate_declaration(text: &str) -> Result<()> {
    let mut c = Cursor::new(text);
    c.expect("xml")?;
    c.required_space()?;
    c.expect("version")?;
    c.space();
    c.expect("=")?;
    c.space();
    let v = c.literal(false, false)?;
    if !v
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(());
    }
    let mut encoding = false;
    let mut standalone = false;
    loop {
        let space = c.space();
        if c.s.is_empty() {
            return Ok(());
        }
        if !space {
            return Err(());
        }
        let n = c.name(false)?;
        c.space();
        c.expect("=")?;
        c.space();
        let value = c.literal(false, false)?;
        match n {
            "encoding" if !encoding && !standalone => {
                encoding = true;
                let mut b = value.bytes();
                if !b.next().is_some_and(|b| b.is_ascii_alphabetic())
                    || !b.all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                {
                    return Err(());
                }
            }
            "standalone" if !standalone => {
                standalone = true;
                if !matches!(value, "yes" | "no") {
                    return Err(());
                }
            }
            _ => return Err(()),
        }
    }
}

pub(super) fn validate_pi(text: &str) -> Result<()> {
    let mut c = Cursor::new(text);
    let target = c.name(false)?;
    if target.eq_ignore_ascii_case("xml") {
        return Err(());
    }
    if !c.s.is_empty() {
        c.required_space()?;
    }
    Ok(())
}
