use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};

use crate::snapshot::Entry;

type DiffMap<'a, 'b> = HashMap<&'a Path, &'b Entry>;
type DiffSet<'a> = HashSet<&'a Path>;

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum Diff {
    Added(PathBuf),
    Removed(PathBuf),
    Changed { left: Entry, right: Entry },
}

impl Diff {
    pub fn as_path(&self) -> &Path {
        match &self {
            Diff::Added(path) => path.as_path(),
            Diff::Removed(path) => path.as_path(),
            Diff::Changed { left, .. } => left.as_ref(),
        }
    }
}

impl Display for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Diff::Added(path) => write!(f, "+ {:?}", path.as_path()),
            Diff::Removed(path) => write!(f, "- {:?}", path.as_path()),
            Diff::Changed { left, right } => {
                write!(f, "! {:?} [{:?} != {:?}]", left.as_path(), left, right)
            }
        }
    }
}

pub fn diff(left: &[Entry], right: &[Entry]) -> HashSet<Diff> {
    let mut only_left: DiffSet<'_> = HashSet::new();
    let mut only_right: DiffMap<'_, '_> = right.iter().map(|it| (it.as_ref(), it)).collect();
    let mut differences = HashSet::new();

    for entry in left {
        let (left_key, left_value) = (entry.as_ref(), entry);

        if let Some(right_value) = only_right.remove(left_key) {
            if right_value != left_value {
                differences.insert(Diff::Changed {
                    left: left_value.clone(),
                    right: right_value.clone(),
                });
            }
        } else {
            only_left.insert(left_key);
        }
    }

    let only_left = only_left
        .iter()
        .map(|it| Diff::Removed(it.to_path_buf()))
        .collect::<Vec<_>>();
    let only_right = only_right
        .keys()
        .map(|it| Diff::Added(it.to_path_buf()))
        .collect::<Vec<_>>();

    differences.extend(only_left);
    differences.extend(only_right);
    differences
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::snapshot::Attributes;

    #[test]
    fn diff_when_same() {
        let attr = Attributes::new(0, 0, 0);
        let left = vec![
            Entry::file("a", attr, "a", 1).unwrap(),
            Entry::file("b", attr, "b", 2).unwrap(),
        ];
        let right = vec![
            Entry::file("a", attr, "a", 1).unwrap(),
            Entry::file("b", attr, "b", 2).unwrap(),
        ];

        let actual = super::diff(&left, &right);

        assert_eq!(actual.is_empty(), true, "should be empty, got {:?}", actual);
    }

    #[test]
    fn diff_when_added() {
        let attr = Attributes::new(0, 0, 0);
        let left = vec![
            Entry::file("a", attr, "a", 1).unwrap(),
            Entry::file("b", attr, "b", 2).unwrap(),
        ];
        let right = vec![
            Entry::file("a", attr, "a", 1).unwrap(),
            Entry::file("b", attr, "b", 2).unwrap(),
            Entry::file("c", attr, "c", 3).unwrap(),
        ];

        let actual = super::diff(&left, &right);

        let mut expected = HashSet::new();
        expected.insert(Diff::Added(PathBuf::from("c")));

        assert_eq!(actual, expected);
    }

    #[test]
    fn diff_when_removed() {
        let attr = Attributes::new(0, 0, 0);
        let left = vec![
            Entry::file("a", attr, "a", 1).unwrap(),
            Entry::file("b", attr, "b", 2).unwrap(),
        ];
        let right = vec![Entry::file("a", attr, "a", 1).unwrap()];

        let actual = super::diff(&left, &right);
        let mut expected = HashSet::new();
        expected.insert(Diff::Removed(PathBuf::from("b")));

        assert_eq!(actual, expected);
    }

    #[test]
    fn diff_when_changed() {
        let attr = Attributes::new(0, 0, 0);
        let original = Entry::file("a", attr, "a", 1).unwrap();
        let changed = Entry::file("a", attr, "changed", 42).unwrap();

        let left = vec![original.clone(), Entry::file("b", attr, "b", 2).unwrap()];
        let right = vec![changed.clone(), Entry::file("b", attr, "b", 2).unwrap()];

        let actual = super::diff(&left, &right);
        let mut expected = HashSet::new();

        expected.insert(Diff::Changed {
            left: original,
            right: changed,
        });

        assert_eq!(actual, expected);
    }
}
