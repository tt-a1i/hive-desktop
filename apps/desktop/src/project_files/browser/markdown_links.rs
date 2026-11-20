/// Resolve a Markdown path against the displayed file's project-relative path.
/// The runtime still checks the destination against the actual worktree.
pub(super) fn relative_markdown_target(source: &str, destination: &str) -> Option<String> {
    if destination.is_empty() || destination.contains(['#', '?']) {
        return None;
    }
    let mut path: Vec<_> = source
        .rsplit_once('/')
        .into_iter()
        .flat_map(|(parent, _)| parent.split('/'))
        .map(str::to_owned)
        .collect();
    let mut parts = destination.split('/').peekable();
    while let Some(part) = parts.next() {
        let part = decode_segment(part)?;
        if part.is_empty()
            || part.contains(['/', '\\', ':'])
            || part.chars().any(char::is_control)
            || part.eq_ignore_ascii_case(".git")
        {
            return None;
        }
        match part.as_str() {
            "." if parts.peek().is_some() => {}
            ".." if parts.peek().is_some() => {
                path.pop()?;
            }
            "." | ".." => return None,
            _ => path.push(part),
        }
    }
    Some(path.join("/"))
}

fn decode_segment(part: &str) -> Option<String> {
    let mut decoded = Vec::with_capacity(part.len());
    let mut bytes = part.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let hex = [bytes.next()?, bytes.next()?];
            if !hex.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            decoded.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
        } else {
            decoded.push(byte);
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::relative_markdown_target;

    #[test]
    fn markdown_links_resolve_relative_to_the_displayed_file() {
        for (source, destination, expected) in [
            (
                "docs/start.md",
                "./guides/setup%20guide.md",
                "docs/guides/setup guide.md",
            ),
            ("docs/guides/setup guide.md", "../start.md", "docs/start.md"),
            ("docs/start.md", "../README.md", "README.md"),
            (
                "docs/100%20/start.md",
                "install.md",
                "docs/100%20/install.md",
            ),
            ("README.md", "中文/caf%C3%A9.md", "中文/café.md"),
            ("README.md", "100%25/%2520%23find+.md", "100%/%20#find+.md"),
        ] {
            assert_eq!(
                relative_markdown_target(source, destination).as_deref(),
                Some(expected),
                "{destination}"
            );
        }
    }

    #[test]
    fn markdown_links_reject_escapes_and_unsupported_destinations() {
        for destination in [
            "",
            "../../outside.md",
            "../%2e%2e/outside.md",
            "/docs/guide.md",
            "//example.com/guide.md",
            "file:///docs/guide.md",
            "javascript:alert(1)",
            "mailto:address@example.com",
            "C:/docs/guide.md",
            "C%3a/docs/guide.md",
            "folder\\file.md",
            "folder%2ffile.md",
            "folder%5cfile.md",
            "../.git/config",
            "../%2EGIT/config",
            "guide.md#heading",
            "#heading",
            "guide.md?query",
            "guide.md%00",
            "guide.md%0a",
            "guide%.md",
            "guide%+1.md",
            "guide%ff.md",
            "guides/",
            "guides/.",
            "guides/..",
        ] {
            assert_eq!(
                relative_markdown_target("docs/start.md", destination),
                None,
                "{destination}"
            );
        }
    }
}
