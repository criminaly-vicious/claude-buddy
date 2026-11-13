//! Minimal line diff for the dry-run preview. Settings files are small, so a
//! plain LCS table is fine.

/// `before` → `after` as lines prefixed with `  `, `- ` or `+ `.
pub fn lines(before: &str, after: &str) -> String {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    // lcs[i][j] = length of the longest common subsequence of a[i..] and b[j..].
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = String::new();
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            out += &format!("  {}\n", a[i]);
            i += 1;
            j += 1;
        } else if i < a.len() && (j == b.len() || lcs[i + 1][j] >= lcs[i][j + 1]) {
            out += &format!("- {}\n", a[i]);
            i += 1;
        } else {
            out += &format!("+ {}\n", b[j]);
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_added_removed_and_kept_lines() {
        assert_eq!(lines("a\nb\nc\n", "a\nx\nc\n"), "  a\n- b\n+ x\n  c\n");
        assert_eq!(lines("", "new\n"), "+ new\n");
        assert_eq!(lines("same\n", "same\n"), "  same\n");
    }
}
