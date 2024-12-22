pub const MAX_LENGTH: usize = 200;

fn footer(m: usize, n: usize) -> String {
    format!("\n\nPage {m}/{n}")
}

/// Remove excessive internal and trailing newlines from a string.
///
/// No, this isn't super algorithmically efficient. That's A-OK here. First, it's unlikely that
/// we're ever going to generate lots of long runs of unwanted whitespace in the first place.
/// Second, our pages are only ever going to be 200 bytes long at most, so this can't ever grown to
/// be O(1_000_000_000**2) or anything like that. I'd vastly rather optimize for simplicity than
/// sophistication here.
fn shrink(text: String) -> String {
    // Pages never have leading whitespace.
    let mut text = text.trim_start().to_string();
    // Lines within a page may have a blank line between them. 2 newlines in a row are fine.
    // More than that are unwanted.
    while text.ends_with("\n\n\n") {
        text.pop();
    }
    // Same if we end up with a string that has a bunch of newlines _inside_ it.
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    // Client apps generally collapse runs of spaces into single spaces. Don't waste bandwidth
    // on those.
    while text.contains("  ") {
        text = text.replace("  ", " ");
    }
    text
}

/// If some smartass makes a post that's longer than the maximum page size, carelessly slice that
/// sucker up into ugly chunks.
pub fn splitted(text: String, max_length: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    // Text will already be shrunk and trimmed before it gets here, so don't bother with doing it
    // again.
    let mut text = text.clone();
    while !text.is_empty() {
        if text.len() <= max_length {
            // If we're lucky enough to coincidentally split between words, trim the space from
            //them.
            out.push(text.trim_end().to_string());
            break;
        } else {
            out.push(text[0..max_length].trim_end().to_string());
            text = text[max_length..].trim_start().to_string();
        }
    }
    out
}

/// Do whatever we reasonably can to batch a series of lines into pages not longer than the
/// maximum Meshtastic message size. If we generate multiple pages, number them so users can
/// reorder them correctly.
pub fn paginate(lines: Vec<String>, max_length: usize) -> Vec<String> {
    let one_page = lines.join("\n");
    if one_page.len() <= max_length {
        return vec![one_page];
    }

    let mut buf = "".to_string();
    let mut pages = Vec::new();

    let page_length = max_length - footer(9, 9).len();

    for line in lines {
        if buf.is_empty() {
            buf = line.clone();
            continue;
        }
        // Before we consider creating a new page, ensure the current one isn't stuffed or tailed
        // with long runs of whitespace.
        buf = shrink(buf);
        if buf.len() + 1 + line.len() > page_length {
            // Pages never need to be trailed by whitespace.
            buf = buf.trim_end().to_string();
            pages.extend(splitted(buf, max_length));
            // Start a new page with the incoming line.
            buf = line.clone();
            continue;
        }
        // Add a new line to the current page.
        buf.push('\n');
        buf.push_str(&line);
    }

    // Does the remaining buffer have anything other than whitespace? Add it as another page.
    buf = shrink(buf).trim_end().to_string();
    if !buf.is_empty() {
        pages.extend(splitted(buf, max_length));
    }

    let page_count = pages.len();
    // If we ended up with more than 1 page, add footers to the end of each.
    if page_count > 1 {
        for (i, buf) in pages.iter_mut().enumerate() {
            buf.push_str(&footer(i + 1, page_count));
        }
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

    #[test]
    fn multi_page_trimmed() {
        let lines = vec![
            "line1\n\n\n\n\n\n\n".to_string(),
            "line2\n\n\n\n\n\n\n\n\n\n\n".to_string(),
        ];
        let pages = paginate(lines, 10);
        assert_eq!(pages, vec!["line1\n\nPage 1/2", "line2\n\nPage 2/2"]);
    }

    #[test]
    fn eat_internal_stacked_newlines() {
        let lines = vec!["line1\n\n\n\n\n\n\n\n\n\n\nline2".to_string()];
        let pages = paginate(lines, 15);
        assert_eq!(pages, vec!["line1\n\nline2"]);
    }

    #[test]
    fn multi_page_trimmed_empty_cdr() {
        let lines = vec!["line1\n\n\n\n\n\n\n\n\n\n".to_string()];
        let pages = paginate(lines, 10);
        assert_eq!(pages, vec!["line1"]);
    }

    #[test]
    fn shrunk() {
        let text = "\n\n\n\nfoo\n\n\n\nbar    baz\n\n\n\n".to_string();
        let trimmed = shrink(text);
        assert_eq!(trimmed, "foo\n\nbar baz\n\n".to_string());
    }

    #[test]
    fn shard() {
        let text = "012345678901234567890123456".to_string();
        let shards = splitted(text, 10);
        assert_eq!(
            shards,
            vec![
                "0123456789".to_string(),
                "0123456789".to_string(),
                "0123456".to_string()
            ]
        )
    }
}
