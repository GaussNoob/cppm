//! "Did you mean" hints for mistyped script and package names.

/// Edit distance counting insertions, deletions, substitutions and swaps of
/// neighbouring characters as one step each.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    // rows[i][j]: distance between the first i characters of a and j of b.
    let mut rows: Vec<Vec<usize>> = (0..=a.len())
        .map(|i| {
            let mut row = vec![0; b.len() + 1];
            row[0] = i;
            row
        })
        .collect();
    rows[0] = (0..=b.len()).collect();
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let substitution = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + substitution);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(rows[i - 2][j - 2] + 1);
            }
            rows[i][j] = best;
        }
    }
    rows[a.len()][b.len()]
}

/// The candidate closest to `input`, when it is close enough to be a typo.
pub fn closest<'a>(
    input: &str,
    candidates: impl IntoIterator<Item = &'a String>,
) -> Option<&'a str> {
    let input = input.to_lowercase();
    candidates
        .into_iter()
        .map(|candidate| (distance(&input, &candidate.to_lowercase()), candidate))
        .filter(|(distance, candidate)| *distance <= (input.len().min(candidate.len()) / 3).max(1))
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate.as_str())
}

/// A parenthetical to append to an "unknown name" error.
pub fn hint<'a>(input: &str, candidates: impl IntoIterator<Item = &'a String>) -> String {
    closest(input, candidates).map_or_else(String::new, |name| format!(" (did you mean '{name}'?)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typos_are_suggested_and_unrelated_names_are_not() {
        let names: Vec<String> = ["dev", "build", "test", "nlohmann_json", "spdlog"]
            .map(String::from)
            .to_vec();
        assert_eq!(closest("biuld", &names), Some("build"));
        assert_eq!(closest("tset", &names), Some("test"));
        assert_eq!(closest("Spdlg", &names), Some("spdlog"));
        assert_eq!(closest("nlohmann-json", &names), Some("nlohmann_json"));
        assert_eq!(closest("deploy", &names), None);
        assert_eq!(hint("dve", &names), " (did you mean 'dev'?)");
        assert_eq!(hint("x", &Vec::new()), "");
    }
}
