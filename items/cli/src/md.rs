//! Markdown for answers.

/// `text` as one table cell.
#[must_use]
pub fn cell(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('|', "\\|")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A table with `headers` and `rows`, each cell escaped.
#[must_use]
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut text = format!("| {} |\n", headers.join(" | "));
    text.push_str(&format!("|{}\n", "---|".repeat(headers.len())));
    for row in rows {
        let cells: Vec<String> = row.iter().map(|c| cell(c)).collect();
        text.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_cells() {
        assert_eq!(
            table(
                &["A", "B"],
                &[vec!["x | y".to_owned(), "two\nlines".to_owned()]]
            ),
            "| A | B |\n|---|---|\n| x \\| y | two lines |\n"
        );
    }
}
