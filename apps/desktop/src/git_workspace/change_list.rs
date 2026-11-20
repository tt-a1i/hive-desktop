use super::Change;
use gpui_kit::Global;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct FolderViews(HashMap<String, HashMap<String, FolderView>>);

impl Global for FolderViews {}

impl FolderViews {
    pub(super) fn get(&self, workspace: &str, root: &str) -> Option<&FolderView> {
        self.0.get(workspace)?.get(root)
    }

    pub(super) fn get_mut(&mut self, workspace: &str, root: &str) -> &mut FolderView {
        self.0
            .entry(workspace.to_owned())
            .or_default()
            .entry(root.to_owned())
            .or_default()
    }
}

#[derive(Default)]
pub(super) struct FolderView {
    pub(super) folded: HashSet<String>,
}

impl FolderView {
    pub(super) fn reveal(&mut self, change: &Change) {
        self.folded.remove(change.dir_name().unwrap_or(""));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row<'a> {
    Folder {
        parent: &'a str,
        count: usize,
        expanded: bool,
    },
    File(usize),
}

pub(super) struct Rows<'a>(pub(super) Vec<Row<'a>>);

impl<'a> Rows<'a> {
    pub(super) fn new(changes: &'a [Change], grouped: bool, view: Option<&FolderView>) -> Self {
        if !grouped {
            return Self((0..changes.len()).map(Row::File).collect());
        }
        let mut positions = HashMap::new();
        let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
        for (index, change) in changes.iter().enumerate() {
            let parent = change.dir_name().unwrap_or("");
            let position = *positions.entry(parent).or_insert_with(|| {
                groups.push((parent, Vec::new()));
                groups.len() - 1
            });
            groups[position].1.push(index);
        }

        let mut rows = Vec::with_capacity(changes.len() + groups.len());
        for (parent, files) in groups {
            let expanded = !view.is_some_and(|view| view.folded.contains(parent));
            rows.push(Row::Folder {
                parent,
                count: files.len(),
                expanded,
            });
            if expanded {
                rows.extend(files.into_iter().map(Row::File));
            }
        }
        Self(rows)
    }

    pub(super) fn files(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        self.0.iter().filter_map(|row| match row {
            Row::File(index) => Some(*index),
            Row::Folder { .. } => None,
        })
    }

    pub(super) fn position(&self, selected: usize) -> Option<usize> {
        self.0.iter().position(|row| *row == Row::File(selected))
    }

    pub(super) fn step(&self, selected: Option<usize>, forward: bool) -> Option<usize> {
        if forward {
            Self::next_file(self.files(), selected)
        } else {
            Self::next_file(self.files().rev(), selected)
        }
    }

    fn next_file(mut files: impl Iterator<Item = usize>, selected: Option<usize>) -> Option<usize> {
        let first = files.next()?;
        let Some(selected) = selected else {
            return Some(first);
        };
        if first != selected {
            files.find(|index| *index == selected);
        }
        files.next().or(Some(first))
    }

    pub(super) fn replacement(
        &self,
        selected: usize,
        outside: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let position = self.position(selected)?;
        self.0[position + 1..]
            .iter()
            .chain(self.0[..position].iter().rev())
            .find_map(|row| match row {
                Row::File(index) if outside(*index) => Some(*index),
                _ => None,
            })
    }
}
