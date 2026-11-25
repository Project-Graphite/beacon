const CONTEXT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Edit {
    Same,
    Removed,
    Added,
}

pub fn unified(before: &str, after: &str, before_label: &str, after_label: &str) -> String {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let edits = edit_script(&old, &new);
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for (index, _) in edits
        .iter()
        .enumerate()
        .filter(|(_, (edit, _))| *edit != Edit::Same)
    {
        let start = index.saturating_sub(CONTEXT);
        let end = (index + 1 + CONTEXT).min(edits.len());
        match hunks.last_mut() {
            Some(last) if start <= last.1 => last.1 = end,
            _ => hunks.push((start, end)),
        }
    }
    if hunks.is_empty() {
        return String::new();
    }
    let old_before = lines_before(&edits, Edit::Added);
    let new_before = lines_before(&edits, Edit::Removed);
    let mut text = format!("--- {before_label}\n+++ {after_label}\n");
    for (start, end) in hunks {
        text.push_str(&format!(
            "@@ -{} +{} @@\n",
            range(old_before[start], old_before[end]),
            range(new_before[start], new_before[end])
        ));
        for (edit, line) in &edits[start..end] {
            let marker = match edit {
                Edit::Same => ' ',
                Edit::Removed => '-',
                Edit::Added => '+',
            };
            text.push(marker);
            text.push_str(&beacon_core::redact(line));
            text.push('\n');
        }
    }
    text
}

fn lines_before(edits: &[(Edit, &str)], other_side: Edit) -> Vec<usize> {
    std::iter::once(0)
        .chain(edits.iter().scan(0, |count, (edit, _)| {
            *count += usize::from(*edit != other_side);
            Some(*count)
        }))
        .collect()
}

fn range(first: usize, end: usize) -> String {
    let count = end - first;
    let start = if count == 0 { first } else { first + 1 };
    format!("{start},{count}")
}

fn edit_script<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<(Edit, &'a str)> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_middle = &old[prefix..old.len() - suffix];
    let new_middle = &new[prefix..new.len() - suffix];
    let (old_len, new_len) = (old_middle.len(), new_middle.len());
    let mut common = vec![vec![0u32; new_len + 1]; old_len + 1];
    for i in (0..old_len).rev() {
        for j in (0..new_len).rev() {
            common[i][j] = if old_middle[i] == new_middle[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    let mut edits: Vec<(Edit, &str)> = old[..prefix]
        .iter()
        .map(|line| (Edit::Same, *line))
        .collect();
    let (mut i, mut j) = (0, 0);
    while i < old_len || j < new_len {
        if i < old_len && j < new_len && old_middle[i] == new_middle[j] {
            edits.push((Edit::Same, old_middle[i]));
            i += 1;
            j += 1;
        } else if i < old_len && (j == new_len || common[i + 1][j] >= common[i][j + 1]) {
            edits.push((Edit::Removed, old_middle[i]));
            i += 1;
        } else {
            edits.push((Edit::Added, new_middle[j]));
            j += 1;
        }
    }
    edits.extend(
        old[old.len() - suffix..]
            .iter()
            .map(|line| (Edit::Same, *line)),
    );
    edits
}

#[cfg(test)]
mod tests {
    use super::unified;

    fn numbered(lines: std::ops::Range<u32>) -> String {
        lines.map(|number| format!("line {number}\n")).collect()
    }

    #[test]
    fn identical_texts_have_no_diff() {
        assert_eq!(unified("a\nb\n", "a\r\nb\r\n", "x", "y"), "");
    }

    #[test]
    fn a_change_shows_three_lines_of_context() {
        let before = numbered(1..11);
        let after = before.replace("line 5\n", "line five\nline 5.5\n");
        assert_eq!(
            unified(&before, &after, "old", "new"),
            "\
--- old
+++ new
@@ -2,7 +2,8 @@
 line 2
 line 3
 line 4
-line 5
+line five
+line 5.5
 line 6
 line 7
 line 8
"
        );
    }

    #[test]
    fn distant_changes_get_separate_hunks_and_close_ones_merge() {
        let before = numbered(1..21);
        let after = before
            .replace("line 2\n", "")
            .replace("line 18\n", "line 18\nline 18.5\n");
        assert_eq!(
            unified(&before, &after, "a", "b"),
            "\
--- a
+++ b
@@ -1,5 +1,4 @@
 line 1
-line 2
 line 3
 line 4
 line 5
@@ -16,5 +15,6 @@
 line 16
 line 17
 line 18
+line 18.5
 line 19
 line 20
"
        );
        let close = before
            .replace("line 5\n", "")
            .replace("line 11\n", "line eleven\n");
        assert_eq!(
            unified(&before, &close, "a", "b"),
            "\
--- a
+++ b
@@ -2,13 +2,12 @@
 line 2
 line 3
 line 4
-line 5
 line 6
 line 7
 line 8
 line 9
 line 10
-line 11
+line eleven
 line 12
 line 13
 line 14
"
        );
    }

    #[test]
    fn creating_and_emptying_a_file() {
        assert_eq!(
            unified("", "{\n}\n", "a", "b"),
            "--- a\n+++ b\n@@ -0,0 +1,2 @@\n+{\n+}\n"
        );
        assert_eq!(
            unified("x\n", "", "a", "b"),
            "--- a\n+++ b\n@@ -1,1 +0,0 @@\n-x\n"
        );
    }
}
