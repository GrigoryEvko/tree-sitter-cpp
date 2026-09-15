//! Compare the facts of Clang with the facts of our tree at the same byte ranges.

use std::collections::{HashMap, HashSet};

use super::label::{Label, is_ambiguous};

/// A category that one side gives to a byte range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact {
    /// The first byte of the range.
    pub begin: u32,
    /// The byte after the range.
    pub end: u32,
    pub label: Label,
    /// The index of the node on its side: a Clang node or a node of our flat tree.
    pub node: u32,
}

/// The result of the comparison of one Clang fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Verdict {
    /// Our tree has the same label at the same range.
    Agree,
    /// Our tree has a label at the same range, and the two labels are the two trees of an ambiguous form.
    Ambiguous,
    /// A mismatch or a missing node in the range of an ambiguous form.
    Nested,
    /// Our tree has labels at the same range, but they are different.
    Mismatch,
    /// Our tree has no label at the range.
    Missing,
}

impl Verdict {
    /// The name of the verdict in the report.
    pub fn name(self) -> &'static str {
        match self {
            Verdict::Agree => "agree",
            Verdict::Ambiguous => "ambiguous",
            Verdict::Nested => "nested",
            Verdict::Mismatch => "mismatch",
            Verdict::Missing => "missing",
        }
    }
}

/// The verdict of one Clang fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub clang: Fact,
    pub verdict: Verdict,
    /// Our fact at the same range: the fact with the same label, the ambiguous fact, or the first fact.
    pub ours: Option<Fact>,
}

/// Give a verdict to each Clang fact. Two Clang facts with the same range and label count one time.
///
/// A mismatch or a missing node in the range of an ambiguous form becomes [`Verdict::Nested`], because
/// the other tree of the form explains it. O(n log n) in the number of facts.
pub fn compare(clang: &[Fact], ours: &[Fact]) -> Vec<Outcome> {
    let mut at: HashMap<(u32, u32), Vec<Fact>> = HashMap::with_capacity(ours.len());
    for fact in ours {
        at.entry((fact.begin, fact.end)).or_default().push(*fact);
    }
    let mut seen = HashSet::with_capacity(clang.len());
    let mut outcomes = Vec::with_capacity(clang.len());
    for fact in clang {
        if !seen.insert((fact.begin, fact.end, fact.label)) {
            continue;
        }
        let (verdict, found) = match at.get(&(fact.begin, fact.end)) {
            None => (Verdict::Missing, None),
            Some(list) => {
                // Our detail `type-first` marks the declaration form. It agrees with the same Clang category.
                let agrees = |ours: &Fact| {
                    ours.label == fact.label
                        || (ours.label.category == fact.label.category
                            && fact.label.detail.is_empty()
                            && ours.label.detail == super::ours::TYPE_FIRST)
                };
                if let Some(same) = list.iter().find(|o| agrees(o)) {
                    (Verdict::Agree, Some(*same))
                } else if let Some(other) = list.iter().find(|o| is_ambiguous(fact.label, o.label)) {
                    (Verdict::Ambiguous, Some(*other))
                } else {
                    (Verdict::Mismatch, list.first().copied())
                }
            }
        };
        outcomes.push(Outcome {
            clang: *fact,
            verdict,
            ours: found,
        });
    }

    let mut regions: Vec<(u32, u32)> = outcomes
        .iter()
        .filter(|o| o.verdict == Verdict::Ambiguous)
        .map(|o| (o.clang.begin, o.clang.end))
        .collect();
    regions.sort_unstable();
    let mut reach = Vec::with_capacity(regions.len());
    let mut farthest = 0;
    for &(_, end) in &regions {
        farthest = farthest.max(end);
        reach.push(farthest);
    }
    for outcome in &mut outcomes {
        if matches!(outcome.verdict, Verdict::Mismatch | Verdict::Missing) {
            let before = regions.partition_point(|&(begin, _)| begin <= outcome.clang.begin);
            if before > 0 && reach[before - 1] >= outcome.clang.end {
                outcome.verdict = Verdict::Nested;
            }
        }
    }
    outcomes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::differ::label::Category;

    /// A fact with no detail and node 0.
    fn fact(begin: u32, end: u32, category: Category) -> Fact {
        Fact {
            begin,
            end,
            label: Label::new(category),
            node: 0,
        }
    }

    #[test]
    fn each_clang_fact_gets_the_verdict_of_our_facts_at_its_range() {
        let clang = [
            fact(0, 10, Category::Compound),
            fact(2, 6, Category::FunctionalCast),
            fact(20, 25, Category::Lambda),
            fact(30, 31, Category::Name),
            fact(30, 31, Category::Name),
        ];
        let ours = [
            fact(0, 10, Category::Compound),
            fact(2, 6, Category::Call),
            fact(20, 25, Category::Subscript),
        ];
        let verdicts: Vec<Verdict> = compare(&clang, &ours).iter().map(|o| o.verdict).collect();
        assert_eq!(
            verdicts,
            [Verdict::Agree, Verdict::Ambiguous, Verdict::Mismatch, Verdict::Missing]
        );
    }

    #[test]
    fn a_disagreement_in_an_ambiguous_range_is_nested() {
        let clang = [
            fact(0, 8, Category::DeclarationStatement),
            fact(0, 7, Category::Variable),
            fact(4, 5, Category::Name),
        ];
        let ours = [fact(0, 8, Category::ExpressionStatement), fact(0, 7, Category::Lambda)];
        let verdicts: Vec<Verdict> = compare(&clang, &ours).iter().map(|o| o.verdict).collect();
        assert_eq!(verdicts, [Verdict::Ambiguous, Verdict::Nested, Verdict::Nested]);
    }
}
