use std::cmp::*;

pub fn sorted(mut patterns: Vec<String>) -> Vec<String> {
    patterns.sort_by(|a, b| {
        let length = a.chars().count().cmp(&b.chars().count());
        if length == Ordering::Equal {
            let ac = a.chars().collect::<Vec<char>>();
            let bc = b.chars().collect::<Vec<char>>();
            for i in 0..ac.len() {
                if ac[i].is_ascii() && bc[i].is_ascii() {
                    let r = ac[i].cmp(&bc[i]);
                    if r != Ordering::Equal {
                        return r;
                    }
                } else if ac[i].is_ascii() && !bc[i].is_ascii() {
                    return Ordering::Greater;
                } else if !ac[i].is_ascii() && bc[i].is_ascii() {
                    return Ordering::Less;
                } else {
                    let r = ac[i].cmp(&bc[i]);
                    if r != Ordering::Equal {
                        return r;
                    }
                }
            }
            return a.cmp(&b);
        }
        return length;
    });
    return patterns;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sorted_1() {
        let vec = vec![String::from("b"), String::from("a")];
        let exp = vec![String::from("a"), String::from("b")];
        assert_eq!(sorted(vec), exp);
    }

    #[test]
    fn test_sorted_2() {
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
}
