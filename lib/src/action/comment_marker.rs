#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentMarker {
    marker: &'static str,
}

impl CommentMarker {
    pub const fn new(marker: &'static str) -> Self {
        Self { marker }
    }

    pub fn marker(&self) -> &'static str {
        self.marker
    }

    pub fn attach(&self, rendered: &str) -> String {
        format!("{rendered}\n\n{}", self.marker)
    }

    pub fn has(&self, body: &str) -> bool {
        body.contains(self.marker)
    }

    pub fn strip<'a>(&self, body: &'a str) -> &'a str {
        match body.find(self.marker) {
            Some(idx) => body[..idx].trim_end(),
            None => body.trim_end(),
        }
    }

    pub fn content_unchanged(&self, existing_body: &str, new_rendered: &str) -> bool {
        self.strip(existing_body) == new_rendered.trim_end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MARKER: &str = "<!-- bofa:test -->";
    const TEST_MARKER_INSTANCE: CommentMarker = CommentMarker::new(TEST_MARKER);

    #[test]
    fn attach_appends_hidden_marker() {
        let body = TEST_MARKER_INSTANCE.attach("report body");
        assert!(body.starts_with("report body"));
        assert!(TEST_MARKER_INSTANCE.has(&body));
        assert!(body.ends_with(TEST_MARKER));
    }

    #[test]
    fn has_detects_marker_substring() {
        assert!(TEST_MARKER_INSTANCE.has("text <!-- bofa:test -->"));
        assert!(!TEST_MARKER_INSTANCE.has("plain text"));
    }

    #[test]
    fn strip_recovers_original_body() {
        let body = TEST_MARKER_INSTANCE.attach("report body");
        assert_eq!(TEST_MARKER_INSTANCE.strip(&body), "report body");
    }

    #[test]
    fn strip_trims_when_marker_absent() {
        assert_eq!(
            TEST_MARKER_INSTANCE.strip("report body  \n\n"),
            "report body"
        );
    }

    #[test]
    fn content_unchanged_compares_marker_stripped_content() {
        let existing = TEST_MARKER_INSTANCE.attach("report body");
        assert!(TEST_MARKER_INSTANCE.content_unchanged(&existing, "report body"));
        assert!(TEST_MARKER_INSTANCE.content_unchanged(&existing, "report body\n\n"));
        assert!(!TEST_MARKER_INSTANCE.content_unchanged(&existing, "different body"));
    }
}
