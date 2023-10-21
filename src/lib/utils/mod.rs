use base64::engine::general_purpose;
use base64::Engine;
use std::cmp::*;
use std::collections::HashSet;

lazy_static! {
    static ref CYRILLIC_UC: HashSet<char> = HashSet::from([
        'А', 'Б', 'В', 'Г', 'Д', 'Е', 'Ё', 'Ж', 'З', 'И', 'Й', 'К', 'Л', 'М', 'Н', 'О', 'П', 'Р',
        'С', 'Т', 'У', 'Ф', 'Х', 'Ц', 'Ч', 'Ш', 'Щ', 'Ъ', 'Ы', 'Ь', 'Э', 'Ю', 'Я'
    ]);
    static ref CYRILLIC_LC: HashSet<char> = HashSet::from([
        'а', 'б', 'в', 'г', 'д', 'е', 'ё', 'ж', 'з', 'и', 'й', 'к', 'л', 'м', 'н', 'о', 'п', 'р',
        'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ъ', 'ы', 'ь', 'э', 'ю', 'я'
    ]);
}

fn is_cyrillic(ch: &char) -> bool {
    CYRILLIC_UC.contains(ch) || CYRILLIC_LC.contains(ch)
}

fn fb2cmp(a: &char, b: &char) -> Ordering {
    if a.is_ascii() && !b.is_ascii() {
        return Ordering::Greater;
    } else if !a.is_ascii() && b.is_ascii() {
        return Ordering::Less;
    } else if !a.is_ascii() && !b.is_ascii() {
        if is_cyrillic(&a) && !is_cyrillic(&b) {
            return Ordering::Less;
        } else if !is_cyrillic(&a) && is_cyrillic(&b) {
            return Ordering::Greater;
        }
    }
    return a.cmp(&b);
}

pub fn fb2sort(lhv: &String, rhv: &String) -> Ordering {
    let mut ac = lhv.chars();
    let mut bc = rhv.chars();

    loop {
        if let Some(a) = ac.next() {
            if let Some(b) = bc.next() {
                let res = fb2cmp(&a, &b);
                if res != Ordering::Equal {
                    return res;
                }
            } else {
                // 'a' is some, but 'b' is none
                return Ordering::Greater;
            }
        } else {
            // 'a' is none, but 'b' not received
            if let Some(_) = bc.next() {
                return Ordering::Less;
            } else {
                return Ordering::Equal;
            }
        }
    }
}

pub fn sorted<T: Into<String>>(patterns: Vec<T>) -> Vec<String> {
    let mut strings = patterns
        .into_iter()
        .map(|value| value.into())
        .collect::<Vec<String>>();
    strings.sort_by(fb2sort);
    return strings;
}

pub fn encode<T: Into<String>>(msg: T) -> String {
    general_purpose::URL_SAFE.encode(msg.into())
}

pub fn decode<T: Into<String>>(msg: T) -> anyhow::Result<String> {
    let decoded = general_purpose::URL_SAFE.decode(msg.into())?;
    let msg = String::from_utf8(decoded)?;
    Ok(msg)
}

pub fn decode_with_lossy<T: Into<String>>(msg: T) -> String {
    match decode(msg) {
        Ok(decoded) => decoded,
        Err(err) => format!("{}", err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sorted_1_and_0() {
        let vec = vec![String::from("b"), String::from("")];
        let exp = vec![String::from(""), String::from("b")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_1_and_1() {
        let vec = vec![String::from("b"), String::from("a")];
        let exp = vec![String::from("a"), String::from("b")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_2_and_1() {
        let vec = vec![String::from("ab"), String::from("a")];
        let exp = vec![String::from("a"), String::from("ab")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_1_and_2() {
        let vec = vec![String::from("b"), String::from("ac")];
        let exp = vec![String::from("ac"), String::from("b")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_2_and_2() {
        let vec = vec![String::from("ab"), String::from("aa")];
        let exp = vec![String::from("aa"), String::from("ab")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_and_ascii_1() {
        let vec = vec![String::from("a"), String::from("я")];
        let exp = vec![String::from("я"), String::from("a")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_and_ascii_2() {
        let vec = vec![String::from("aя"), String::from("яя")];
        let exp = vec![String::from("яя"), String::from("aя")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_lc() {
        let vec = vec![String::from("яя"), String::from("ая")];
        let exp = vec![String::from("ая"), String::from("яя")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_uc() {
        let vec = vec![String::from("ЫЫ"), String::from("АЫ")];
        let exp = vec![String::from("АЫ"), String::from("ЫЫ")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_mc() {
        let vec = vec![String::from("Ыа"), String::from("АЫ")];
        let exp = vec![String::from("АЫ"), String::from("Ыа")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_case_1() {
        let vec = vec![String::from("Кя"), String::from("Ка")];
        let exp = vec![String::from("Ка"), String::from("Кя")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_cyr_case_2() {
        let vec = vec![
            String::from("Дви"),
            String::from("Дво"),
            String::from("Дву"),
            String::from("Два"),
        ];
        let exp = vec![
            String::from("Два"),
            String::from("Дви"),
            String::from("Дво"),
            String::from("Дву"),
        ];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_encode_decode() {
        let orig = String::from("Some Message content");
        let encoded: String = encode(&orig);
        assert_eq!(orig, decode(&encoded).unwrap());
    }
}
