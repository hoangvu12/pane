//! Names as file search matches them: folded (letter case and accents
//! ignored, "Résumé" finds "resume") and split into words, and the score
//! of a name against a query.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// `text` lowercased and without accents: its compatibility decomposition
/// without combining marks, then lowercased.
pub(crate) fn fold(text: &str) -> String {
    if text.is_ascii() {
        return text.to_ascii_lowercase();
    }
    let mut folded = String::with_capacity(text.len());
    for c in text.nfkd() {
        if !is_combining_mark(c) {
            folded.extend(c.to_lowercase());
        }
    }
    folded
}

/// The words of a name, folded: each run of letters and digits, and, where
/// a run changes from lower to upper case or between letters and digits
/// ("invoiceMay2026"), each part of it as well, without repeats.
pub(crate) fn words(name: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut push = |word: &str| {
        let word = fold(word);
        if !word.is_empty() && !words.contains(&word) {
            words.push(word);
        }
    };
    let mut run_start: Option<usize> = None;
    let mut parts: Vec<(usize, usize)> = Vec::new();
    let mut part_start = 0;
    let mut previous: Option<char> = None;
    let chars: Vec<(usize, char)> = name.char_indices().collect();
    for (at, &(index, c)) in chars.iter().enumerate() {
        if c.is_alphanumeric() {
            match (run_start, previous) {
                (None, _) => {
                    run_start = Some(index);
                    part_start = index;
                }
                (Some(_), Some(before)) if splits(before, c) => {
                    parts.push((part_start, index));
                    part_start = index;
                }
                _ => {}
            }
            previous = Some(c);
        } else if let Some(start) = run_start.take() {
            end_run(name, start, index, part_start, &mut parts, &mut push);
            previous = None;
        }
        if at + 1 == chars.len()
            && let Some(start) = run_start.take()
        {
            end_run(name, start, name.len(), part_start, &mut parts, &mut push);
        }
    }
    words
}

/// Ends a run of letters and digits: the run as one word, then its parts if
/// it has more than one.
fn end_run(
    name: &str,
    start: usize,
    end: usize,
    part_start: usize,
    parts: &mut Vec<(usize, usize)>,
    push: &mut impl FnMut(&str),
) {
    push(&name[start..end]);
    if !parts.is_empty() {
        parts.push((part_start, end));
        for &(from, to) in parts.iter() {
            push(&name[from..to]);
        }
    }
    parts.clear();
}

/// Whether a word ends between `before` and `c`: lower to upper case, or
/// between a letter and a digit.
fn splits(before: char, c: char) -> bool {
    (before.is_lowercase() && c.is_uppercase())
        || (before.is_alphabetic() && c.is_numeric())
        || (before.is_numeric() && c.is_alphabetic())
}

/// The words of a query, folded, split only where letters and digits stop
/// (a query is not split by case), without repeats.
pub(crate) fn query_words(query: &str) -> Vec<String> {
    let folded = fold(query);
    let mut words: Vec<String> = Vec::new();
    for word in folded.split(|c: char| !c.is_alphanumeric()) {
        if !word.is_empty() && !words.iter().any(|known| known == word) {
            words.push(word.to_owned());
        }
    }
    words
}

/// A query ready to score names against.
pub(crate) struct Prepared {
    /// The whole query, folded and trimmed.
    pub(crate) text: String,
    pub(crate) words: Vec<String>,
}

impl Prepared {
    pub(crate) fn new(query: &str) -> Prepared {
        Prepared {
            text: fold(query.trim()),
            words: query_words(query),
        }
    }
}

/// What a candidate is scored on.
pub(crate) struct Candidate<'a> {
    /// Its own name.
    pub(crate) name: &'a str,
    /// The folders it is in, below its root, outermost first.
    pub(crate) folders: &'a [&'a str],
    pub(crate) is_folder: bool,
    /// Last modified, in seconds since 1970.
    pub(crate) modified: u64,
}

/// Ranks of a match, best first; the score's integer part.
const EXACT_NAME: f64 = 100.0;
const EXACT_STEM: f64 = 90.0;
const NAME_PREFIX: f64 = 80.0;
const WORDS_IN_NAME: f64 = 60.0;
const WORDS_IN_FOLDERS: f64 = 40.0;

/// The score of `candidate` for `query`, higher is better, or `None` when
/// a word of the query starts no word of its name or folders. The shape is
/// Raycast's (#126, "Matching and ranking"): an exact name above an exact
/// stem, above a name prefix, above every word starting a word of the name,
/// above words found only in the folders; a match only in the extension
/// scores lower; then a little for recent changes, decaying with age, a
/// little for a folder, words in order, and a little less for depth.
pub(crate) fn score(query: &Prepared, candidate: &Candidate<'_>, now: u64) -> Option<f64> {
    if query.words.is_empty() {
        return None;
    }
    let name = fold(candidate.name);
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], Some(&name[dot + 1..])),
        _ => (name.as_str(), None),
    };
    let name_words = words(candidate.name);
    let mut folder_words: Vec<String> = Vec::new();
    for folder in candidate.folders {
        for word in words(folder) {
            if !folder_words.contains(&word) {
                folder_words.push(word);
            }
        }
    }
    let mut score = if name == query.text {
        EXACT_NAME
    } else if stem == query.text {
        EXACT_STEM
    } else if name.starts_with(&query.text) {
        NAME_PREFIX
    } else {
        let mut only_in_folders = 0usize;
        let mut only_extension = 0usize;
        let mut positions = Vec::with_capacity(query.words.len());
        for word in &query.words {
            let in_name = name_words.iter().position(|known| known.starts_with(word));
            match in_name {
                Some(at) => {
                    positions.push(at);
                    let matches_stem = name_words
                        .iter()
                        .any(|known| known.starts_with(word) && Some(known.as_str()) != extension);
                    if !matches_stem {
                        only_extension += 1;
                    }
                }
                None if folder_words.iter().any(|known| known.starts_with(word)) => {
                    only_in_folders += 1;
                }
                None => return None,
            }
        }
        let in_order = positions.windows(2).all(|pair| pair[0] <= pair[1]);
        let base = if only_in_folders == 0 {
            WORDS_IN_NAME
        } else {
            WORDS_IN_FOLDERS - 4.0 * (only_in_folders - 1) as f64
        };
        base + if in_order { 3.0 } else { 0.0 } - 8.0 * only_extension as f64
    };
    // Recent changes: up to 6, halving every 30 days.
    let age_days = now.saturating_sub(candidate.modified) as f64 / 86_400.0;
    if candidate.modified > 0 {
        score += 6.0 * 0.5f64.powf(age_days / 30.0);
    }
    if candidate.is_folder {
        score += 1.5;
    }
    score -= 0.25 * candidate.folders.len().min(24) as f64;
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_ignores_case_and_accents() {
        assert_eq!(fold("Résumé PLAN ü"), "resume plan u");
        assert_eq!(fold("ÅNGSTRÖM"), "angstrom");
        assert_eq!(fold("plain"), "plain");
    }

    #[test]
    fn a_name_splits_into_words_and_their_parts() {
        assert_eq!(
            words("invoiceMay2026.pdf"),
            ["invoicemay2026", "invoice", "may", "2026", "pdf"]
        );
        assert_eq!(words("Résumé plan ü.txt"), ["resume", "plan", "u", "txt"]);
        assert_eq!(words("IMG_0042.JPG"), ["img", "0042", "jpg"]);
        assert_eq!(words("..."), Vec::<String>::new());
        assert_eq!(words("a-a"), ["a"]);
    }

    #[test]
    fn a_query_is_split_where_letters_and_digits_stop() {
        assert_eq!(
            query_words("Invoices 2026/may"),
            ["invoices", "2026", "may"]
        );
        assert_eq!(query_words("  "), Vec::<String>::new());
    }

    fn scored(query: &str, name: &str, folders: &[&str]) -> Option<f64> {
        let query = Prepared::new(query);
        let candidate = Candidate {
            name,
            folders,
            is_folder: false,
            modified: 0,
        };
        score(&query, &candidate, 0)
    }

    #[test]
    fn exact_names_rank_above_stems_prefixes_words_and_folders() {
        let exact = scored("report.pdf", "report.pdf", &[]).unwrap();
        let stem = scored("report", "report.pdf", &[]).unwrap();
        let prefix = scored("repo", "report.pdf", &[]).unwrap();
        let word = scored("final", "report final.pdf", &[]).unwrap();
        let folder = scored("invoices report", "report.pdf", &["Invoices"]).unwrap();
        assert!(exact > stem && stem > prefix && prefix > word && word > folder);
        assert_eq!(scored("missing", "report.pdf", &["Invoices"]), None);
    }

    #[test]
    fn accents_and_case_are_ignored_and_the_extension_alone_scores_lower() {
        assert!(scored("resume", "Résumé plan ü.txt", &[]).is_some());
        let in_stem = scored("pdf", "pdf notes.txt", &[]).unwrap();
        let in_extension = scored("pd", "notes.pdf", &[]).unwrap();
        assert!(in_stem > in_extension);
    }

    #[test]
    fn recent_and_shallow_entries_rank_a_little_higher() {
        let query = Prepared::new("plan");
        let now = 400 * 86_400;
        let at = |modified, folders: &'static [&'static str]| {
            score(
                &query,
                &Candidate {
                    name: "plan b.txt",
                    folders,
                    is_folder: false,
                    modified,
                },
                now,
            )
            .unwrap()
        };
        assert!(at(now, &[]) > at(now - 300 * 86_400, &[]));
        assert!(at(now, &[]) > at(now, &["a", "b", "c"]));
    }
}
