extern crate opds_db_api;

#[macro_use]
extern crate lazy_static;

pub mod opds;
pub mod utils;
pub mod statistic;

pub fn search_by_mask<F, S>(mask: S, fetcher: F) -> anyhow::Result<(Vec<String>, Vec<String>)>
where
    F: Fn(&String) -> anyhow::Result<Vec<String>>,
    S: Into<String>,
{
    let mut mask = mask.into();
    let mut complete = Vec::new();
    let mut incomplete = Vec::new();

    loop {
        let patterns = fetcher(&mask)?;
        let (mut exact, mut tail) = patterns.into_iter().partition(|curr| mask.eq(curr));
        complete.append(&mut exact);

        if tail.is_empty() {
            break;
        } else if 1 == tail.len() {
            std::mem::swap(&mut mask, &mut tail[0]);
        } else if 2 == tail.len() {
            let are_equal = tail[0].to_lowercase() == tail[1].to_lowercase();
            if are_equal {
                std::mem::swap(&mut mask, &mut tail[0]);
            } else {
                incomplete.append(&mut tail);
                break;
            }
        } else {
            incomplete.append(&mut tail);
            break;
        }
    }

    Ok((complete, incomplete))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fetcher(mask: &String) -> anyhow::Result<Vec<String>> {
        let out = match mask.as_str() {
            "A" => vec!["A", "Ab", "Ac"],
            "B" => vec!["B", "BB"],
            "BB" => vec!["BBB"],
            "BBB" => vec!["BBBB"],
            "BBBB" => vec!["BBBB"],
            "C" => vec!["CC", "cc"],
            "CC" => vec!["CCC", "ccc"],
            "CCC" => vec!["CCC", "ccc"],
            "ccc" => vec!["ccc"],
            _ => vec![],
        };
        if out.is_empty() {
            Err(anyhow::anyhow!("Unexpected mask '{mask}'"))
        } else {
            Ok(out.into_iter().map(|s| String::from(s)).collect())
        }
    }

    #[test]
    fn test_a() -> anyhow::Result<()> {
        let (exact, tail) = search_by_mask("A", fetcher)?;
        assert_eq!(
            vec!["A"],
            exact.iter().map(|a| a.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(
            vec!["Ab", "Ac"],
            tail.iter().map(|a| a.as_str()).collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn test_b() -> anyhow::Result<()> {
        let empty: Vec<&str> = Vec::new();
        let (exact, tail) = search_by_mask("B", fetcher)?;
        assert_eq!(
            vec!["B", "BBBB"],
            exact.iter().map(|a| a.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(empty, tail.iter().map(|a| a.as_str()).collect::<Vec<_>>());
        Ok(())
    }

    #[test]
    fn test_c() -> anyhow::Result<()> {
        let empty: Vec<&str> = Vec::new();
        let (exact, tail) = search_by_mask("C", fetcher)?;
        assert_eq!(
            vec!["CCC", "ccc"],
            exact.iter().map(|a| a.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(empty, tail.iter().map(|a| a.as_str()).collect::<Vec<_>>());
        Ok(())
    }
}
