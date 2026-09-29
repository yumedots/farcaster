pub(crate) fn invocation_token(token: &str) -> &str {
    token.trim_end_matches(['.', ',', ';', ':', '!', '?'])
}

pub(super) fn tokens(input: &str) -> impl Iterator<Item = (usize, usize, &str)> {
    let mut start = None;
    input
        .char_indices()
        .chain(std::iter::once((input.len(), ' ')))
        .filter_map(move |(index, character)| {
            if character.is_whitespace() {
                let token_start = start.take()?;
                Some((token_start, index, &input[token_start..index]))
            } else {
                start.get_or_insert(index);
                None
            }
        })
}

#[cfg(test)]
#[path = "prompt_fragments_tests.rs"]
mod tests;
