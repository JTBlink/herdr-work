//! Original Sidebar search and Quick Open, shared by both frontends.
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::{Regex, RegexBuilder};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickFile {
    pub path: PathBuf,
    pub label: String,
    pub label_lower: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentHit {
    pub path: PathBuf,
    pub label: String,
    pub line: usize,
    pub context: String,
    pub matches: Vec<(usize, usize)>,
}

pub const QUICK_OPEN_FILE_LIMIT: usize = 20_000;
pub const CONTENT_SEARCH_MATCH_LIMIT: usize = 1_000;
pub const CONTENT_SEARCH_FILE_LIMIT: usize = 20_000;
pub const CONTENT_SEARCH_MAX_BYTES: u64 = 1024 * 1024;

pub fn collect_quick_files(root: &Path, show_hidden: bool, limit: usize) -> (Vec<QuickFile>, bool) {
    let mut files = Vec::new();
    let mut truncated = false;
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .hidden(!show_hidden)
        .follow_links(false)
        .require_git(false)
        .filter_entry(|entry| entry.file_name() != ".git");
    for entry in builder.build().flatten() {
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.into_path();
        let label = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let label_lower = label.to_lowercase();
        files.push(QuickFile {
            path,
            label,
            label_lower,
        });
        if files.len() >= limit {
            truncated = true;
            break;
        }
    }
    files.sort_by(|left, right| left.label_lower.cmp(&right.label_lower));
    (files, truncated)
}

pub fn collect_content_hits(
    root: &Path,
    show_hidden: bool,
    query: &str,
    include: &str,
    exclude: &str,
    options: SearchOptions,
    limit: usize,
) -> Result<(Vec<ContentHit>, bool), String> {
    if query.is_empty() || limit == 0 {
        return Ok((Vec::new(), false));
    }
    let pattern = if options.regex {
        let pattern = if options.whole_word {
            format!(r"\b(?:{query})\b")
        } else {
            query.to_string()
        };
        Some(
            RegexBuilder::new(&pattern)
                .case_insensitive(!options.match_case)
                .build()
                .map_err(|error| format!("Invalid regular expression: {error}"))?,
        )
    } else {
        None
    };
    let includes = build_search_globs(include, "include")?;
    let excludes = build_search_globs(exclude, "exclude")?;
    let (files, file_limit_reached) =
        collect_quick_files(root, show_hidden, CONTENT_SEARCH_FILE_LIMIT);
    let mut hits = Vec::new();
    for file in files {
        if includes
            .as_ref()
            .is_some_and(|patterns| !patterns.is_match(&file.label))
            || excludes
                .as_ref()
                .is_some_and(|patterns| patterns.is_match(&file.label))
        {
            continue;
        }
        if std::fs::metadata(&file.path)
            .map(|metadata| metadata.len() > CONTENT_SEARCH_MAX_BYTES)
            .unwrap_or(true)
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(&file.path) else {
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            let ranges = content_line_match_ranges(line, query, options, pattern.as_ref());
            let matched = pattern
                .as_ref()
                .is_some_and(|pattern| pattern.is_match(line))
                || !ranges.is_empty();
            if !matched {
                continue;
            }
            let (context, matches) = content_hit_context(line, &ranges);
            hits.push(ContentHit {
                path: file.path.clone(),
                label: file.label.clone(),
                line: index + 1,
                context,
                matches,
            });
            if hits.len() >= limit {
                return Ok((hits, true));
            }
        }
    }
    Ok((hits, file_limit_reached))
}

pub fn build_search_globs(raw: &str, label: &str) -> Result<Option<GlobSet>, String> {
    let patterns = raw
        .split(',')
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .collect::<Vec<_>>();
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            Glob::new(pattern).map_err(|error| format!("Invalid {label} pattern: {error}"))?;
        builder.add(glob);
        if !pattern.contains(['*', '?', '[', '{']) {
            let directory = pattern.trim_end_matches(['/', '\\']);
            let descendant = format!("{directory}/**");
            builder.add(
                Glob::new(&descendant)
                    .map_err(|error| format!("Invalid {label} pattern: {error}"))?,
            );
            if !directory.contains(['/', '\\']) {
                let nested_descendant = format!("**/{directory}/**");
                builder.add(
                    Glob::new(&nested_descendant)
                        .map_err(|error| format!("Invalid {label} pattern: {error}"))?,
                );
            }
        }
    }
    builder
        .build()
        .map(Some)
        .map_err(|error| format!("Invalid {label} pattern: {error}"))
}

pub fn content_line_match_ranges(
    line: &str,
    query: &str,
    options: SearchOptions,
    pattern: Option<&Regex>,
) -> Vec<(usize, usize)> {
    if let Some(pattern) = pattern {
        return pattern
            .find_iter(line)
            .filter(|found| !found.is_empty())
            .map(|found| (found.start(), found.end()))
            .collect();
    }
    literal_match_ranges(line, query, options)
}

pub fn literal_match_ranges(
    line: &str,
    query: &str,
    options: SearchOptions,
) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    if options.match_case {
        return line
            .match_indices(query)
            .filter_map(|(start, matched)| {
                let end = start + matched.len();
                word_boundary_matches(line, start, end, options.whole_word).then_some((start, end))
            })
            .collect();
    }

    let lowered = line.to_lowercase();
    let mut source_by_lowered_character = Vec::new();
    for (source_start, character) in line.char_indices() {
        let source_range = (source_start, source_start + character.len_utf8());
        source_by_lowered_character.extend(std::iter::repeat_n(
            source_range,
            character.to_lowercase().count(),
        ));
    }
    let mut lowered_boundaries = lowered
        .char_indices()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    lowered_boundaries.push(lowered.len());
    if source_by_lowered_character.len() + 1 != lowered_boundaries.len() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    let mut ranges = Vec::new();
    for (lowered_start, matched) in lowered.match_indices(&needle) {
        let lowered_end = lowered_start + matched.len();
        if !word_boundary_matches(&lowered, lowered_start, lowered_end, options.whole_word) {
            continue;
        }
        let Ok(first) = lowered_boundaries.binary_search(&lowered_start) else {
            continue;
        };
        let Ok(after_last) = lowered_boundaries.binary_search(&lowered_end) else {
            continue;
        };
        if first >= after_last {
            continue;
        }
        let range = (
            source_by_lowered_character[first].0,
            source_by_lowered_character[after_last - 1].1,
        );
        if ranges.last() != Some(&range) {
            ranges.push(range);
        }
    }
    ranges
}

pub fn word_boundary_matches(haystack: &str, start: usize, end: usize, whole_word: bool) -> bool {
    if !whole_word {
        return true;
    }
    let before = haystack[..start].chars().next_back();
    let after = haystack[end..].chars().next();
    !before.is_some_and(search_word_char) && !after.is_some_and(search_word_char)
}

pub fn content_hit_context(line: &str, ranges: &[(usize, usize)]) -> (String, Vec<(usize, usize)>) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return (String::new(), Vec::new());
    }
    let start = line.len() - line.trim_start().len();
    let end = start + trimmed.len();
    let ranges = ranges
        .iter()
        .filter_map(|&(match_start, match_end)| {
            if match_start >= end || match_end <= start {
                return None;
            }
            let match_start = match_start.max(start);
            let match_end = match_end.min(end);
            Some((match_start - start, match_end - start))
        })
        .collect();
    (trimmed.replace('\t', " "), ranges)
}

pub fn search_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

pub fn quick_matches(files: &[QuickFile], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..files.len()).collect();
    }
    let query_lower = query.to_lowercase();
    let mut ranked = files
        .iter()
        .enumerate()
        .filter_map(|(index, file)| {
            fuzzy_score_lowercased(&query_lower, &file.label_lower).map(|score| (index, score))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_index, left_score), (right_index, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| {
                files[*left_index]
                    .label
                    .len()
                    .cmp(&files[*right_index].label.len())
            })
            .then_with(|| files[*left_index].label.cmp(&files[*right_index].label))
    });
    ranked.into_iter().map(|(index, _)| index).collect()
}

pub fn fuzzy_score_lowercased(query: &str, candidate: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let mut wanted = query.chars();
    let mut current = wanted.next()?;
    let mut score = 0i64;
    let mut previous_match = None;
    let mut previous_char = None;
    for (index, ch) in candidate.chars().enumerate() {
        if ch == current {
            score += 10;
            if previous_match == Some(index.saturating_sub(1)) {
                score += 8;
            }
            if index == 0
                || previous_char
                    .is_some_and(|before| matches!(before, '/' | '\\' | '-' | '_' | '.'))
            {
                score += 6;
            }
            previous_match = Some(index);
            match wanted.next() {
                Some(next) => current = next,
                None => return Some(score - candidate.chars().count() as i64),
            }
        }
        previous_char = Some(ch);
    }
    None
}

/// Uses the same line matcher as content search. Regex replacements support
/// `$1` / `${name}` captures; literal replacement text is never expanded.
pub fn replace_content(
    text: &str,
    query: &str,
    replacement: &str,
    options: SearchOptions,
    preserve_case: bool,
    only_line: Option<usize>,
) -> Result<(String, usize), String> {
    if query.is_empty() {
        return Err("搜索内容不能为空".into());
    }
    let pattern = if options.regex {
        let pattern = if options.whole_word {
            format!(r"\b(?:{query})\b")
        } else {
            query.to_owned()
        };
        Some(
            RegexBuilder::new(&pattern)
                .case_insensitive(!options.match_case)
                .build()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let mut output = String::new();
    let mut count = 0;
    for (index, part) in text.split_inclusive('\n').enumerate() {
        if only_line.is_some_and(|line| line != index + 1) {
            output.push_str(part);
            continue;
        }
        let line = part.trim_end_matches(['\r', '\n']);
        let mut at = 0;
        let mut changes = Vec::new();
        if let Some(pattern) = &pattern {
            for captures in pattern.captures_iter(line) {
                let found = captures.get(0).unwrap();
                if found.is_empty() {
                    continue;
                }
                let mut expanded = String::new();
                captures.expand(replacement, &mut expanded);
                changes.push((found.start(), found.end(), expanded));
            }
        } else {
            changes.extend(
                literal_match_ranges(line, query, options)
                    .into_iter()
                    .map(|(a, b)| (a, b, replacement.to_owned())),
            );
        }
        if only_line.is_some() {
            changes.truncate(1);
        }
        for (start, end, mut value) in changes {
            output.push_str(&line[at..start]);
            if preserve_case {
                value = replacement_case(&line[start..end], &value);
            }
            output.push_str(&value);
            at = end;
            count += 1;
        }
        output.push_str(&part[at..]);
    }
    Ok((output, count))
}

fn replacement_case(matched: &str, replacement: &str) -> String {
    let letters: Vec<_> = matched
        .chars()
        .filter(|c| {
            c.is_alphabetic() && c.to_lowercase().to_string() != c.to_uppercase().to_string()
        })
        .collect();
    if letters.is_empty() {
        return replacement.to_owned();
    }
    if letters.iter().all(|c| c.is_uppercase()) {
        return replacement.to_uppercase();
    }
    if letters.iter().all(|c| c.is_lowercase()) {
        return replacement.to_lowercase();
    }
    if letters[0].is_uppercase() && letters[1..].iter().all(|c| c.is_lowercase()) {
        let mut chars = replacement.chars();
        return chars
            .next()
            .map(|c| c.to_uppercase().to_string() + &chars.as_str().to_lowercase())
            .unwrap_or_default();
    }
    replacement.to_owned()
}

#[cfg(test)]
mod replacement_tests {
    use super::*;
    #[test]
    fn literal_unicode_case_and_line_endings_follow_search_matches() {
        let (text, count) = replace_content(
            "Foo FOO foo food 中文\r\nfoo\r\n",
            "foo",
            "bar",
            SearchOptions {
                whole_word: true,
                ..Default::default()
            },
            true,
            None,
        )
        .unwrap();
        assert_eq!(text, "Bar BAR bar food 中文\r\nbar\r\n");
        assert_eq!(count, 4);
        assert_eq!(
            replace_content(
                "中文中文",
                "中文",
                "$1",
                SearchOptions::default(),
                false,
                None
            )
            .unwrap(),
            ("$1$1".into(), 2)
        );
    }
    #[test]
    fn regex_captures_and_invalid_patterns() {
        let options = SearchOptions {
            regex: true,
            ..Default::default()
        };
        assert_eq!(
            replace_content("item-12", r"item-(\d+)", "value-$1", options, false, None).unwrap(),
            ("value-12".into(), 1)
        );
        assert!(replace_content("text", "[", "", options, false, None).is_err());
    }
}
