use aho_corasick::{AhoCorasick, BuildError};
use itertools::Itertools;

use crate::{source_hier::SourceFileID, StatementsInFile};


/// Identifies a single log statement: the file it is in and its index in that file's log statements
pub(crate) type StatementID = (SourceFileID, usize);


#[derive(Debug)]
pub(crate) struct AhoPrefilter {
    ac: AhoCorasick,
    ids: Vec<StatementID>,
}

impl AhoPrefilter {
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

        Ok(AhoPrefilter { ac, ids })
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
