//! Scrapers for the data Woot embeds in an offer page but does not expose
//! through its GraphQL API.
//!
//! Both values live in inline `<script>` JSON rather than markup, so these
//! match against the raw page text. The patterns tolerate an optional
//! backslash before each quote (`\\?"`) so they work whether the page is read
//! decoded or still JSON-escaped inside a tls-client envelope.

use std::sync::LazyLock;

use regex::Regex;

/// The review count Woot renders into its rating widget.
static TOTAL_REVIEWS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\\?"TotalReviewCount\\?"\s*:\s*(\d+)"#).expect("invalid TotalReviewCount pattern")
});

/// The Amazon ASIN, anchored to `RatingSummaryData` so an `Asin` key belonging
/// to some other blob on the page cannot be picked up by mistake.
static ASIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)RatingSummaryData\s*=\s*\[.*?\\?"Asin\\?"\s*:\s*\\?"([A-Z0-9]{10})\\?""#)
        .expect("invalid Asin pattern")
});

/// Extracts the total review count, or `None` if the page does not carry one.
pub fn total_reviews(html: &str) -> Option<u32> {
    TOTAL_REVIEWS
        .captures(html)?
        .get(1)?
        .as_str()
        .parse::<u32>()
        .ok()
}

/// Extracts the Amazon ASIN, or `None` if the page does not carry one.
pub fn asin(html: &str) -> Option<String> {
    Some(ASIN.captures(html)?.get(1)?.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_total_reviews_from_decoded_html() {
        let html = r#"<script>var d = {"TotalReviewCount":1234,"Rating":4.5};</script>"#;
        assert_eq!(total_reviews(html), Some(1234));
    }

    #[test]
    fn reads_total_reviews_from_json_escaped_html() {
        let html = r#"{"body":"<script>var d = {\"TotalReviewCount\":1234};</script>"}"#;
        assert_eq!(total_reviews(html), Some(1234));
    }

    #[test]
    fn tolerates_whitespace_around_the_review_count() {
        let html = r#"{"TotalReviewCount" : 42}"#;
        assert_eq!(total_reviews(html), Some(42));
    }

    #[test]
    fn returns_no_review_count_when_absent() {
        assert_eq!(
            total_reviews("<html><body>no reviews here</body></html>"),
            None
        );
    }

    #[test]
    fn returns_no_review_count_when_not_a_number() {
        assert_eq!(total_reviews(r#"{"TotalReviewCount":"many"}"#), None);
    }

    #[test]
    fn reads_asin_from_decoded_html() {
        let html = r#"<script>RatingSummaryData = [{"Asin":"B08N5WRWNW","Rating":4}];</script>"#;
        assert_eq!(asin(html), Some("B08N5WRWNW".to_string()));
    }

    #[test]
    fn reads_asin_from_json_escaped_html() {
        let html = r#"{"body":"RatingSummaryData = [{\"Asin\":\"B08N5WRWNW\"}]"}"#;
        assert_eq!(asin(html), Some("B08N5WRWNW".to_string()));
    }

    #[test]
    fn finds_asin_across_newlines() {
        let html = "RatingSummaryData = [\n  {\n    \"Asin\": \"B0123ABCDE\"\n  }\n]";
        assert_eq!(asin(html), Some("B0123ABCDE".to_string()));
    }

    #[test]
    fn ignores_an_asin_that_precedes_the_rating_summary() {
        let html = r#"{"Asin":"AAAAAAAAAA"} ... RatingSummaryData = [{"Asin":"B08N5WRWNW"}]"#;
        assert_eq!(asin(html), Some("B08N5WRWNW".to_string()));
    }

    #[test]
    fn returns_no_asin_without_the_rating_summary_anchor() {
        assert_eq!(asin(r#"{"Asin":"B08N5WRWNW"}"#), None);
    }

    #[test]
    fn returns_no_asin_when_the_value_is_not_ten_characters() {
        assert_eq!(asin(r#"RatingSummaryData = [{"Asin":"B08N5"}]"#), None);
    }

    #[test]
    fn returns_no_asin_when_absent() {
        assert_eq!(asin("<html><body>no asin here</body></html>"), None);
    }
}
