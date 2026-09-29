use super::*;

#[test]
fn tokens_split_on_whitespace_with_byte_ranges() {
    let input = "$commit now";
    assert_eq!(
        tokens(input).collect::<Vec<_>>(),
        [(0, 7, "$commit"), (8, 11, "now")]
    );
}

#[test]
fn invocation_token_strips_trailing_punctuation_only() {
    for suffix in [".", ",", ";", ":", "!", "?"] {
        let token = format!("$commit{suffix}");
        assert_eq!(invocation_token(&token), "$commit");
    }
    assert_eq!(invocation_token("$commit.md"), "$commit.md");
    assert_eq!(invocation_token("$commit-extra."), "$commit-extra");
}
