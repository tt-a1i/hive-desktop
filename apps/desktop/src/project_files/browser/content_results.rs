use super::{Match, SearchFlags, SearchHit};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Pick {
    File(String),
    Hit(Match),
}

impl Pick {
    pub(super) fn path(&self) -> &str {
        match self {
            Self::File(path) => path,
            Self::Hit(hit) => &hit.path,
        }
    }
}

pub(super) enum Row<'a> {
    File {
        path: &'a str,
        count: usize,
        expanded: bool,
    },
    Hit(usize, &'a SearchHit),
}

impl Row<'_> {
    pub(super) fn pick(&self) -> Pick {
        match self {
            Self::File { path, .. } => Pick::File((*path).to_owned()),
            Self::Hit(_, hit) => Pick::Hit(hit.selection.clone()),
        }
    }

    fn matches(&self, selected: &Pick) -> bool {
        match (self, selected) {
            (Self::File { path, .. }, Pick::File(selected)) => *path == selected,
            (Self::Hit(_, hit), Pick::Hit(selected)) => hit.selection == *selected,
            _ => false,
        }
    }
}

#[derive(Default)]
pub(super) struct ContentResults {
    key: Option<ContentQuery>,
    folded: HashSet<String>,
    pub(super) cursor: Option<Pick>,
}

#[derive(PartialEq, Eq)]
pub(super) struct ContentQuery {
    pub(super) scope_generation: u64,
    pub(super) flags: SearchFlags,
    pub(super) text: String,
}

impl ContentResults {
    pub(super) fn set_query(&mut self, key: Option<ContentQuery>) {
        if self.key != key {
            self.key = key;
            self.folded.clear();
            self.cursor = None;
        }
    }

    pub(super) fn rows<'a>(&self, hits: &'a [SearchHit]) -> Vec<Row<'a>> {
        let mut rows = Vec::new();
        let mut start = 0;
        for group in hits.chunk_by(|left, right| left.selection.path == right.selection.path) {
            let path = group[0].selection.path.as_str();
            let expanded = !self.folded.contains(path);
            rows.push(Row::File {
                path,
                count: group.len(),
                expanded,
            });
            if expanded {
                rows.extend(
                    group
                        .iter()
                        .enumerate()
                        .map(|(index, hit)| Row::Hit(start + index, hit)),
                );
            }
            start += group.len();
        }
        rows
    }

    pub(super) fn position(rows: &[Row<'_>], cursor: &Pick) -> Option<usize> {
        rows.iter().position(|row| row.matches(cursor))
    }

    pub(super) fn hit_visible(&self, hits: &[SearchHit], selected: &Match) -> bool {
        !self.folded.contains(&selected.path) && hits.iter().any(|hit| hit.selection == *selected)
    }

    pub(super) fn reconcile(&mut self, hits: &[SearchHit]) {
        let rows = self.rows(hits);
        if self
            .cursor
            .as_ref()
            .is_some_and(|cursor| Self::position(&rows, cursor).is_some())
        {
            return;
        }
        self.cursor = rows
            .iter()
            .find(|row| matches!(row, Row::Hit(..)))
            .or_else(|| rows.first())
            .map(Row::pick);
    }

    pub(super) fn move_cursor(
        &mut self,
        hits: &[SearchHit],
        delta: isize,
        hits_only: bool,
    ) -> Option<usize> {
        let rows = self.rows(hits);
        let current = self
            .cursor
            .as_ref()
            .and_then(|pick| Self::position(&rows, pick));
        let choices: Vec<_> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| !hits_only || matches!(row, Row::Hit(..)))
            .collect();
        let first = choices.first()?;
        let last = choices.last()?;
        let (index, row) = match current {
            Some(current) => {
                let before = choices.partition_point(|(index, _)| *index < current);
                if choices
                    .get(before)
                    .is_some_and(|(index, _)| *index == current)
                {
                    choices[before.saturating_add_signed(delta).min(choices.len() - 1)]
                } else if delta > 0 {
                    *choices.get(before).unwrap_or(last)
                } else {
                    choices[before.saturating_sub(1)]
                }
            }
            None if delta < 0 => *last,
            None => *first,
        };
        self.cursor = Some(row.pick());
        Some(index)
    }

    pub(super) fn toggle(&mut self, path: &str, expand: Option<bool>) {
        if expand.unwrap_or_else(|| self.folded.contains(path)) {
            self.folded.remove(path);
        } else {
            self.folded.insert(path.to_owned());
        }
        self.cursor = Some(Pick::File(path.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(path: &str, line: usize) -> SearchHit {
        SearchHit {
            selection: Match {
                path: path.to_owned(),
                line: Some(line),
            },
            snippet: None,
        }
    }

    fn query(scope: u64, case_sensitive: bool, text: &str) -> Option<ContentQuery> {
        Some(ContentQuery {
            scope_generation: scope,
            flags: SearchFlags {
                case_sensitive,
                whole_word: false,
            },
            text: text.to_owned(),
        })
    }

    #[test]
    fn keyboard_search_projects_visible_rows_to_original_hits() {
        let hits = [
            hit("api/README.md", 3),
            hit("api/README.md", 350),
            hit("ui/README.md", 9),
        ];
        let mut state = ContentResults::default();
        state.set_query(query(1, false, "needle"));
        state.reconcile(&hits);
        assert_eq!(state.cursor, Some(Pick::Hit(hits[0].selection.clone())));
        assert_eq!(state.move_cursor(&hits, 1, false), Some(2));
        assert_eq!(state.cursor, Some(Pick::Hit(hits[1].selection.clone())));
        state.toggle("api/README.md", Some(false));
        assert_eq!(state.cursor, Some(Pick::File("api/README.md".into())));
        assert!(!state.hit_visible(&hits, &hits[1].selection));
        assert_eq!(state.move_cursor(&hits, 1, false), Some(1));
        assert_eq!(state.cursor, Some(Pick::File("ui/README.md".into())));
        assert_eq!(state.move_cursor(&hits, 1, true), Some(2));
        let rows = state.rows(&hits);
        assert!(matches!(
            rows[0],
            Row::File {
                path: "api/README.md",
                count: 2,
                expanded: false
            }
        ));
        assert!(matches!(rows[2], Row::Hit(2, hit) if hit.selection.line == Some(9)));
    }

    #[test]
    fn folds_cross_pages_but_reset_with_scope_query_or_flags() {
        let next_page = [hit("api/README.md", 620), hit("ui/README.md", 701)];
        let mut state = ContentResults::default();
        state.set_query(query(1, false, "needle"));
        state.toggle("api/README.md", Some(false));
        state.set_query(query(1, false, "needle"));
        state.reconcile(&next_page);
        assert_eq!(state.cursor, Some(Pick::File("api/README.md".into())));
        assert!(matches!(
            state.rows(&next_page)[0],
            Row::File {
                count: 1,
                expanded: false,
                ..
            }
        ));
        for identity in [
            query(2, false, "needle"),
            query(2, true, "needle"),
            query(2, true, "other"),
            Some(ContentQuery {
                scope_generation: 2,
                flags: SearchFlags {
                    case_sensitive: true,
                    whole_word: true,
                },
                text: "other".to_owned(),
            }),
            None,
        ] {
            state.set_query(identity);
            state.reconcile(&next_page);
            assert_eq!(
                state.cursor,
                Some(Pick::Hit(next_page[0].selection.clone()))
            );
            assert!(state.hit_visible(&next_page, &next_page[0].selection));
            state.toggle("api/README.md", Some(false));
        }
    }
}
