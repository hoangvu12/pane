//! Matching and ranking for root search.
//!
//! A query is matched against each result's title and subtitle, ignoring
//! letter case and the spaces around words. Every word of the query must
//! appear in the title or subtitle. Matches are ranked by how well the
//! title matches, best first:
//!
//! 1. the title is the query;
//! 2. the title starts with the query;
//! 3. every word of the query starts a word of the title;
//! 4. every word of the query appears in the title;
//! 5. otherwise (some word appears only in the subtitle).
//!
//! Results that rank the same keep their order in root search. This is a
//! deliberately simple first ranking, not tuned relevance: no typo
//! tolerance, abbreviations, frequency or recency.

/// How well a result matches a query; lower is better.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    Exact,
    Prefix,
    WordPrefixes,
    InTitle,
    InSubtitle,
}

/// A query as it is matched: lowercase, its words separated by single
/// spaces. An empty query matches everything.
pub(crate) struct Query {
    text: String,
    words: Vec<String>,
}

impl Query {
    pub(crate) fn new(query: &str) -> Query {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        Query {
            text: words.join(" "),
            words,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// How well a result with `title` and `subtitle` matches; `None` if it
    /// does not.
    fn rank(&self, title: &str, subtitle: Option<&str>) -> Option<Rank> {
        let title = title.to_lowercase();
        let subtitle = subtitle.map(str::to_lowercase).unwrap_or_default();
        let title_words: Vec<&str> = title
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect();
        let rank = if title == self.text {
            Rank::Exact
        } else if title.starts_with(&self.text) {
            Rank::Prefix
        } else if self.words.iter().all(|word| {
            title_words
                .iter()
                .any(|title_word| title_word.starts_with(word.as_str()))
        }) {
            Rank::WordPrefixes
        } else if self.words.iter().all(|word| title.contains(word.as_str())) {
            Rank::InTitle
        } else if self
            .words
            .iter()
            .all(|word| title.contains(word.as_str()) || subtitle.contains(word.as_str()))
        {
            Rank::InSubtitle
        } else {
            return None;
        };
        Some(rank)
    }
}

/// The indices of the `results` (title, subtitle) that match `query`, best
/// match first; equally good matches, and every result for an empty query,
/// keep their order.
pub(crate) fn rank<'a>(
    query: &Query,
    results: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) -> Vec<usize> {
    let mut matches: Vec<(Rank, usize)> = results
        .into_iter()
        .enumerate()
        .filter_map(|(index, (title, subtitle))| {
            if query.is_empty() {
                return Some((Rank::Exact, index));
            }
            query.rank(title, subtitle).map(|rank| (rank, index))
        })
        .collect();
    // Stable, so equal ranks keep root search order.
    matches.sort_by_key(|&(rank, _)| rank);
    matches.into_iter().map(|(_, index)| index).collect()
}
