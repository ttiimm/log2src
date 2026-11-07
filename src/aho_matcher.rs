use aho_corasick::{AhoCorasick, BuildError};
use itertools::Itertools;

use crate::{root_matcher::StatementID, StatementsInFile};

#[derive(Debug)]
pub(crate) struct AhoMatcher {
    ac: AhoCorasick,
    ids: Vec<StatementID>,
}

impl AhoMatcher {
    pub(crate) fn new<'a>(
        files: impl Iterator<Item = &'a StatementsInFile>,
    ) -> Result<Self, BuildError> {
        let mut longest = Vec::new();
        let mut ids = Vec::new();
        for file in files {
            for (idx, stmt) in file.log_statements.iter().enumerate() {
                let longest_literal = stmt.literals.iter().max_by_key(|s| s.len());
                if let Some(literal) = longest_literal {
                    longest.push(literal);
                    ids.push((file.id, idx));
                }
            }
        }

        let ac = AhoCorasick::new(longest)?;

        Ok(AhoMatcher { ac, ids })
    }

    pub(crate) fn candidates<'a>(
        &'a self,
        body: &'a str,
    ) -> impl Iterator<Item = StatementID> + 'a {
        let matches = self.ac.find_overlapping_iter(body);
        matches
            .into_iter()
            .map(move |m| self.ids[m.pattern().as_usize()])
            .unique()
    }
}

#[cfg(test)]
mod tests {
    use aho_corasick::{MatchKind, PatternID};

    use super::*;

    #[test]
    fn test_aho_basics() {
        let patterns = &["apple", "maple", "Snapple"];
        let haystack = "Nobody likes maple in their apple flavored Snapple.";

        let ac = AhoCorasick::new(patterns).unwrap();
        let mut matches = vec![];

        for mat in ac.find_iter(haystack) {
            matches.push((mat.pattern(), mat.start(), mat.end()));
        }

        assert_eq!(
            matches,
            vec![
                (PatternID::must(1), 13, 18),
                (PatternID::must(0), 28, 33),
                (PatternID::must(2), 43, 50),
            ]
        );

        let name = "tim";
        println!("{name} your name is {} your age {}", "hi", 2);

        // log statements
        let patterns = &["Sam", "Samwise"];
        // log message
        let haystack = "Samwise";

        let ac = AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostLongest)
            .build(patterns)
            .unwrap();
        let mat = ac.find(haystack).expect("should have a match");
        assert_eq!("Samwise", &haystack[mat.start()..mat.end()]);
    }
}
