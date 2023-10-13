use std::cmp::*;

pub fn sorter(a: &String, b: &String) -> Ordering {
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
}

fn fb2cmp(a: &char, b: &char) -> Ordering {
    if a.is_ascii() && !b.is_ascii() {
        return Ordering::Greater;
    } else if !a.is_ascii() && b.is_ascii() {
        return Ordering::Less;
    }
    let r = a.cmp(&b);
    // println!("{a} {r:?} {b}");
    return r;
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

pub fn sorted(mut patterns: Vec<String>) -> Vec<String> {
    // println!("<= {patterns:?}");
    patterns.sort_by(fb2sort);
    // println!("=> {patterns:?}");
    return patterns;
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
}
