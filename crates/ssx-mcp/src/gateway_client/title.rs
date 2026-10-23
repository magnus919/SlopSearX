//! Python3.14 Unicode16 default tool title, derived from retained Unicode data.
//! Accepted source_pool property/lower tables retain their source attribution.
#[path = "title_data.rs"]
mod data;
fn flags(c: char) -> u8 {
    let cp = c as u32;
    let at = data::PROPERTIES.partition_point(|row| row.0 <= cp);
    if at == 0 {
        0
    } else {
        let (_, end, flags) = data::PROPERTIES[at - 1];
        if cp <= end { flags } else { 0 }
    }
}
fn mapped(out: &mut String, c: char, table: &[(u32, &str)]) {
    if let Ok(at) = table.binary_search_by_key(&(c as u32), |r| r.0) {
        out.push_str(table[at].1)
    } else {
        out.push(c)
    }
}
pub fn default_title(name: &str) -> String {
    let input = name.replace(['_', '-'], " ");
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut previous = false;
    for (i, &c) in chars.iter().enumerate() {
        if !previous {
            mapped(&mut out, c, data::TITLE)
        } else if c == 'Σ' {
            let before = chars[..i]
                .iter()
                .rev()
                .find(|&&c| flags(c) & 4 == 0)
                .is_some_and(|&c| flags(c) & 2 != 0);
            let after = chars[i + 1..]
                .iter()
                .find(|&&c| flags(c) & 4 == 0)
                .is_some_and(|&c| flags(c) & 2 != 0);
            out.push(if before && !after { 'ς' } else { 'σ' });
        } else {
            mapped(&mut out, c, data::LOWER)
        }
        previous = flags(c) & 2 != 0;
    }
    out
}

pub fn identifier(name: &str) -> bool {
    fn xid(c: char, flag: u8) -> bool {
        let cp = c as u32;
        let at = data::IDENTIFIER.partition_point(|row| row.0 <= cp);
        at > 0 && cp <= data::IDENTIFIER[at - 1].1 && data::IDENTIFIER[at - 1].2 & flag != 0
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || xid(first, 1))
        && chars.all(|c| xid(c, 2))
        && !matches!(
            name,
            "False"
                | "None"
                | "True"
                | "and"
                | "as"
                | "assert"
                | "async"
                | "await"
                | "break"
                | "class"
                | "continue"
                | "def"
                | "del"
                | "elif"
                | "else"
                | "except"
                | "finally"
                | "for"
                | "from"
                | "global"
                | "if"
                | "import"
                | "in"
                | "is"
                | "lambda"
                | "nonlocal"
                | "not"
                | "or"
                | "pass"
                | "raise"
                | "return"
                | "try"
                | "while"
                | "with"
                | "yield"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_unicode_title_expansions_digraphs_sigma_and_combining_boundaries() {
        assert_eq!(default_title("hello_world-x"), "Hello World X");
        assert_eq!(default_title("ǳ_ß_ΟΣ"), "ǲ Ss Ος");
        assert_eq!(default_title("a\u{301}b"), "A\u{301}B");
    }
}

#[cfg(test)]
mod identifier_tests {
    use super::*;
    #[test]
    fn original_generated_signature_accepts_unicode16_identifiers_but_not_keywords_or_schema_hyphens()
     {
        assert!(identifier("query"));
        assert!(identifier("Δείγμα"));
        assert!(identifier("a\u{301}"));
        assert!(!identifier("class"));
        assert!(!identifier("location-name"));
        assert!(!identifier("2query"));
    }
}
