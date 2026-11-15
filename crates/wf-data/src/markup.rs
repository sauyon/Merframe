pub(crate) fn without_markup(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut in_markup = false;
    for character in text.chars() {
        match character {
            '<' => in_markup = true,
            '>' => in_markup = false,
            _ if !in_markup => plain.push(character),
            _ => {}
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_dropped() {
        assert_eq!(
            without_markup("Kill |COUNT| Enemies with <DT_POISON>Toxin Damage."),
            "Kill |COUNT| Enemies with Toxin Damage."
        );
    }
}
