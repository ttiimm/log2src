use regex::{RegexSet, RegexSetBuilder};

use crate::{source_hier::SourceFileID, StatementsInFile};

/// Identifies a single log statement: the file it is in and its index in that file's log statements
pub(crate) type StatementID = (SourceFileID, usize);


#[derive(Debug)]
pub(crate) struct RootMatcher {
    set: RegexSet,
    ids: Vec<StatementID>,
}

impl RootMatcher {
    pub(crate) fn new<'a>(
        files: impl Iterator<Item = &'a StatementsInFile>,
    ) -> Result<Self, regex::Error> {
        let mut patterns = Vec::new();
        let mut ids = Vec::new();
        for file in files {
            for (idx, stmt) in file.log_statements.iter().enumerate() {
                patterns.push(stmt.pattern_str.as_str());
                ids.push((file.id, idx));
            }
        }

        let set = RegexSetBuilder::new(&patterns)
            // 256 MB
            .size_limit(256 << 20)
            .dfa_size_limit(256 << 20)
            .build()?;
        Ok(RootMatcher { set, ids })
    }

    pub(crate) fn candidates<'a>(&'a self, body: &str) -> impl Iterator<Item = StatementID> + 'a {
        // self.set.matches(body).iter().map(|i| self.ids[i])
        let matches = self.set.matches(body);
        matches.into_iter().map(move |i| self.ids[i])
    }
}
