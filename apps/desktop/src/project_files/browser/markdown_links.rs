use markdown::{ParseOptions, mdast::Node, to_mdast};
use std::{collections::HashMap, ops::Range};

#[derive(Default)]
pub(super) struct MarkdownHeadings {
    ranges: HashMap<String, Range<usize>>,
    outline: Vec<MarkdownOutlineHeading>,
}

pub(super) struct MarkdownOutlineHeading {
    pub(super) title: String,
    pub(super) level: u8,
    pub(super) range: Range<usize>,
}

impl MarkdownHeadings {
    pub(super) fn parse(source: &str) -> Self {
        // Match TextView's default Markdown blocks, including fenced math.
        let mut options = ParseOptions::gfm();
        options.constructs.math_flow = true;
        options.constructs.math_text = true;
        let Ok(root) = to_mdast(source, &options) else {
            return Self::default();
        };
        let mut headings = Self::default();
        let mut next_suffix = HashMap::new();
        let mut nodes = vec![&root];
        while let Some(node) = nodes.pop() {
            if let Node::Heading(heading) = node
                && let Some(position) = &heading.position
            {
                let text = heading_text(&heading.children);
                let base = heading_slug(&text);
                let mut slug = base.clone();
                if headings.ranges.contains_key(&slug) {
                    let suffix = next_suffix.entry(base.clone()).or_insert(1);
                    while headings.ranges.contains_key(&slug) {
                        slug = format!("{base}-{suffix}");
                        *suffix += 1;
                    }
                }
                let range = position.start.offset..position.end.offset;
                headings.ranges.insert(slug, range.clone());
                if heading.depth <= 3 {
                    let title = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !title.is_empty() {
                        headings.outline.push(MarkdownOutlineHeading {
                            title,
                            level: heading.depth,
                            range,
                        });
                    }
                }
            }
            if let Some(children) = node.children() {
                nodes.extend(children.iter().rev());
            }
        }
        headings
    }

    pub(super) fn outline(&self) -> Option<&[MarkdownOutlineHeading]> {
        (self.outline.len() >= 2).then_some(&self.outline)
    }

    pub(super) fn range(&self, fragment: &str) -> Option<Range<usize>> {
        let slug = percent_decode(fragment)?;
        if slug.is_empty() {
            return None;
        }
        self.ranges.get(&slug).cloned()
    }
}

fn heading_text(children: &[Node]) -> String {
    let mut title = String::new();
    let mut nodes: Vec<_> = children.iter().rev().collect();
    while let Some(node) = nodes.pop() {
        match node {
            Node::Text(text) => title.push_str(&text.value),
            Node::InlineCode(code) => title.push_str(&code.value),
            Node::InlineMath(math) => title.push_str(&math.value),
            _ => {
                if let Some(children) = node.children() {
                    nodes.extend(children.iter().rev());
                }
            }
        }
    }
    title
}

fn heading_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|character| {
            let character = if character == ' ' { '-' } else { character };
            (character == '-'
                || unicode_ident::is_xid_continue(character)
                || character.is_alphanumeric())
            .then_some(character)
        })
        .collect()
}

pub(super) fn is_markdown_file(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
}

/// Resolve a link against the displayed file's project-relative path.
/// The runtime still checks the destination against the actual worktree.
pub(super) fn relative_markdown_target<'a>(
    source: &str,
    destination: &'a str,
) -> Option<(String, Option<&'a str>)> {
    let (destination, fragment) = destination
        .split_once('#')
        .map_or((destination, None), |(path, fragment)| {
            (path, Some(fragment))
        });
    if destination.is_empty() || destination.contains('?') {
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
        let part = percent_decode(part)?;
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
    let path = path.join("/");
    if fragment.is_some() && !is_markdown_file(&path) {
        return None;
    }
    Some((path, fragment.filter(|fragment| !fragment.is_empty())))
}

fn percent_decode(part: &str) -> Option<String> {
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
    use super::{MarkdownHeadings, relative_markdown_target};

    #[test]
    fn heading_links_resolve_parser_positions_and_unique_slugs() {
        let source = concat!(
            "# **Repeat**\n\n",
            "```md\n# Repeat\n# Fake heading\n```\n\n",
            "$$\n# Math heading\n$$\n\n",
            "# Repeat-1\n\n",
            "Repeat\n------\n\n",
            "> ## [Café](./notes.md) & 章節\n\n",
            "## देवनागरी\n\n",
            "### Read <em>this</em> `value`.\n",
        );
        let headings = MarkdownHeadings::parse(source);
        for (fragment, expected) in [
            ("repeat", "# **Repeat**"),
            ("repeat-1", "# Repeat-1"),
            ("repeat-2", "Repeat\n------"),
            (
                "caf%C3%A9--%E7%AB%A0%E7%AF%80",
                "## [Café](./notes.md) & 章節",
            ),
            ("देवनागरी", "## देवनागरी"),
            ("read-this-value", "### Read <em>this</em> `value`."),
        ] {
            assert_eq!(
                headings.range(fragment).and_then(|range| source.get(range)),
                Some(expected),
                "{fragment}"
            );
        }
        for fragment in [
            "",
            "fake-heading",
            "math-heading",
            "missing",
            "%",
            "%FF",
            "repeat%252D1",
        ] {
            assert_eq!(headings.range(fragment), None, "{fragment}");
        }
    }

    #[test]
    fn outline_keeps_heading_order_depth_and_exact_duplicate_positions() {
        let source = concat!(
            "# **Repeat**\n\n",
            "```md\n## Fake heading\n```\n\n",
            "$$\n### Math heading\n$$\n\n",
            "#### Repeat\n\n",
            "Repeat\n------\n\n",
            "> ### [中文](./notes.md) `Café`\n",
        );
        let headings = MarkdownHeadings::parse(source);
        let outline = headings.outline().unwrap();
        assert_eq!(
            outline
                .iter()
                .map(|heading| (
                    heading.title.as_str(),
                    heading.level,
                    &source[heading.range.clone()]
                ))
                .collect::<Vec<_>>(),
            [
                ("Repeat", 1, "# **Repeat**"),
                ("Repeat", 2, "Repeat\n------"),
                ("中文 Café", 3, "### [中文](./notes.md) `Café`"),
            ]
        );
        assert_eq!(Some(outline[1].range.clone()), headings.range("repeat-2"));
        assert_eq!(
            Some(outline[2].range.clone()),
            headings.range("%E4%B8%AD%E6%96%87-caf%C3%A9")
        );
    }

    #[test]
    fn outline_requires_two_nonempty_headings_within_its_levels() {
        let headings =
            MarkdownHeadings::parse("## Only\n\n#### Deeper\n\n###\n\n```\n## Fake\n```");
        assert!(headings.outline().is_none());
        assert!(headings.range("deeper").is_some());
    }

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
                relative_markdown_target(source, destination),
                Some((expected.to_owned(), None)),
                "{destination}"
            );
        }
    }

    #[test]
    fn markdown_link_fragments_are_separate_from_the_scoped_destination() {
        for (link, path, fragment) in [
            (
                "./setup%20guide.md#linux",
                "docs/setup guide.md",
                Some("linux"),
            ),
            (
                "../GUIDE.MARKDOWN#repeat-1",
                "GUIDE.MARKDOWN",
                Some("repeat-1"),
            ),
            (
                "./spec%23v2%2emd#%E7%AB%A0%E7%AF%80",
                "docs/spec#v2.md",
                Some("%E7%AB%A0%E7%AF%80"),
            ),
            ("./setup%20guide.md#", "docs/setup guide.md", None),
        ] {
            assert_eq!(
                relative_markdown_target("docs/start.md", link),
                Some((path.to_owned(), fragment)),
                "{link}"
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
            "#heading",
            "guide.txt#heading",
            "guide.md?query#heading",
            "../../outside.md#heading",
            "../.git/guide.md#heading",
            "folder%2fguide.md#heading",
            "https://example.com/guide.md#heading",
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
