use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::snapshot::Entry;

type DiffMap<'a, 'b> = HashMap<&'a Path, &'b Entry>;
type DiffSet<'a> = HashSet<&'a Path>;

pub fn diff<'a, 'b, 'c>(left: &'a [Entry], right: &'b [Entry]) -> HashSet<&'c Path>
where
    'a: 'c,
    'b: 'c,
{
    let mut only_left: DiffSet<'_> = HashSet::new();
    let mut only_right: DiffMap<'_, '_> = right.iter().map(|it| (it.as_ref(), it)).collect();
    let mut differences: DiffSet<'_> = HashSet::new();

    for entry in left {
        let (left_key, left_value) = (entry.as_ref(), entry);

        if let Some(right_value) = only_right.remove(left_key) {
            if right_value != left_value {
                differences.insert(left_key);
            }
        } else {
            only_left.insert(left_key);
        }
    }

    differences.extend(only_left);
    differences.extend(only_right.keys());
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
        expected.insert(Path::new("c"));

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
        expected.insert(Path::new("b"));

        assert_eq!(actual, expected);
    }

    #[test]
    fn diff_when_changed() {
        let attr = Attributes::new(0, 0, 0);
        let left = vec![
            Entry::file("a", attr, "a", 1).unwrap(),
            Entry::file("b", attr, "b", 2).unwrap(),
        ];
        let right = vec![
            Entry::file("a", attr, "changed", 42).unwrap(),
            Entry::file("b", attr, "b", 2).unwrap(),
        ];

        let actual = super::diff(&left, &right);
        let mut expected = HashSet::new();
        expected.insert(Path::new("a"));

        assert_eq!(actual, expected);
    }
}
