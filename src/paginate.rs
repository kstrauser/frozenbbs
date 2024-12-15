pub type CommandOut = Vec<String>;

const MAX_LENGTH: usize = 200;

fn footer(m: usize, n: usize) -> String {
    format!("\n\nPage {}/{}", m, n)
}

pub fn paginate(lines: CommandOut, max_length: usize) -> Vec<String> {
    let one_page = lines.join("\n");
    if one_page.len() <= max_length {
        return vec![one_page];
    }

    let mut buf = "".to_string();
    let mut pages = Vec::new();

    let page_length = max_length - footer(9, 9).len();

    for line in lines {
        if buf.is_empty() {
            buf = line.to_string();
            continue;
        }
        if buf.len() + 1 + line.len() > page_length {
            pages.push(buf);
            buf = line.to_string();
            continue;
        }
        buf.push('\n');
        buf.push_str(&line);
    }
    if !buf.is_empty() {
        pages.push(buf);
    }

    let page_count = pages.len();
    for (i, buf) in pages.iter_mut().enumerate() {
        buf.push_str(&footer(i + 1, page_count));
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_page() {
        let lines = vec!["line1".to_string(), "line2".to_string()];
        let pages = paginate(lines, 20);
        assert_eq!(pages, vec!["line1\nline2"]);
    }

    #[test]
    fn multi_page() {
        let lines = vec!["line1".to_string(), "line2".to_string()];
        let pages = paginate(lines, 10);
        assert_eq!(pages, vec!["line1\n\nPage 1/2", "line2\n\nPage 2/2"]);
    }
}
