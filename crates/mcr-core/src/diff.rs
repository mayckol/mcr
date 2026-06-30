use crate::hunk::{Category, Origin};
use similar::{ChangeTag, TextDiff};

/// Whitespace handling applied during tokenization (FR-014).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhitespaceMode {
    None,
    IgnoreTrailing,
    IgnoreAll,
}

impl WhitespaceMode {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s {
            Some("ignore_trailing") => WhitespaceMode::IgnoreTrailing,
            Some("ignore_all") => WhitespaceMode::IgnoreAll,
            _ => WhitespaceMode::None,
        }
    }

    fn normalize(&self, line: &str) -> String {
        match self {
            WhitespaceMode::None => line.to_string(),
            WhitespaceMode::IgnoreTrailing => line.trim_end().to_string(),
            WhitespaceMode::IgnoreAll => line.chars().filter(|c| !c.is_whitespace()).collect(),
        }
    }
}

/// One classified slice of the three-way merge over base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Region {
    /// Lines identical across all three versions.
    Stable {
        local: (usize, usize),
        incoming: (usize, usize),
    },
    /// A divergent block.
    Change {
        base: (usize, usize),
        local: (usize, usize),
        incoming: (usize, usize),
        origin: Origin,
        category: Category,
    },
}

/// Map every base line index to the matched line index in `other`, if any
/// (built from the equal runs of a line diff over normalized text).
fn match_map(base: &[&str], other: &[&str]) -> Vec<Option<usize>> {
    let mut m = vec![None; base.len()];
    let diff = TextDiff::from_slices(base, other);
    for op in diff.ops() {
        if matches!(op.tag(), similar::DiffTag::Equal) {
            let old = op.old_range();
            let new = op.new_range();
            for (k, bi) in old.clone().enumerate() {
                m[bi] = Some(new.start + k);
            }
        }
    }
    m
}

fn classify(base: &[String], local: &[String], incoming: &[String]) -> (Origin, Category, bool) {
    let local_changed = local != base;
    let incoming_changed = incoming != base;
    let conflict = local_changed && incoming_changed && local != incoming;

    if conflict {
        return (Origin::Both, Category::Conflicting, true);
    }
    // Non-conflicting: exactly one side changed, or both changed identically.
    let (origin, changed) = if local_changed && incoming_changed {
        (Origin::Both, local) // both-same change
    } else if local_changed {
        (Origin::Local, local)
    } else {
        (Origin::Incoming, incoming)
    };
    let category = if base.is_empty() {
        Category::Added
    } else if changed.is_empty() {
        Category::Removed
    } else {
        Category::Modified
    };
    (origin, category, false)
}

/// Three-way (diff3) decomposition of base/local/incoming into ordered regions.
pub fn diff3(
    base_lines: &[String],
    local_lines: &[String],
    incoming_lines: &[String],
    mode: WhitespaceMode,
) -> Vec<Region> {
    let nb: Vec<String> = base_lines.iter().map(|l| mode.normalize(l)).collect();
    let nl: Vec<String> = local_lines.iter().map(|l| mode.normalize(l)).collect();
    let nr: Vec<String> = incoming_lines.iter().map(|l| mode.normalize(l)).collect();

    let nb_ref: Vec<&str> = nb.iter().map(|s| s.as_str()).collect();
    let nl_ref: Vec<&str> = nl.iter().map(|s| s.as_str()).collect();
    let nr_ref: Vec<&str> = nr.iter().map(|s| s.as_str()).collect();
    let ml = match_map(&nb_ref, &nl_ref);
    let mr = match_map(&nb_ref, &nr_ref);

    let mut regions = Vec::new();
    let (mut bi, mut li, mut ri) = (0usize, 0usize, 0usize);

    let push_change =
        |regions: &mut Vec<Region>, b: (usize, usize), l: (usize, usize), r: (usize, usize)| {
            let bs = &nb[b.0..b.1];
            let ls = &nl[l.0..l.1];
            let rs = &nr[r.0..r.1];
            if ls == bs && rs == bs {
                return; // no real change (can occur with empty spans)
            }
            let (origin, category, _conflict) = classify(bs, ls, rs);
            regions.push(Region::Change {
                base: b,
                local: l,
                incoming: r,
                origin,
                category,
            });
        };

    while bi < nb.len() {
        if ml[bi] == Some(li) && mr[bi] == Some(ri) {
            // Stable line; extend the run.
            let bstart = bi;
            let lstart = li;
            let rstart = ri;
            while bi < nb.len() && ml[bi] == Some(li) && mr[bi] == Some(ri) {
                bi += 1;
                li += 1;
                ri += 1;
            }
            let _ = bstart;
            regions.push(Region::Stable {
                local: (lstart, li),
                incoming: (rstart, ri),
            });
        } else {
            let bstart = bi;
            let lstart = li;
            let rstart = ri;
            // Advance to the next base line that is matched on both sides at/after
            // the current side cursors — that is the next stable anchor.
            let mut bj = bi;
            let (lend, rend) = loop {
                if bj >= nb.len() {
                    break (nl.len(), nr.len());
                }
                if let (Some(lt), Some(rt)) = (ml[bj], mr[bj]) {
                    if lt >= li && rt >= ri {
                        break (lt, rt);
                    }
                }
                bj += 1;
            };
            push_change(&mut regions, (bstart, bj), (lstart, lend), (rstart, rend));
            bi = bj;
            li = lend;
            ri = rend;
        }
    }

    // Trailing insertions after base is exhausted.
    if li < nl.len() || ri < nr.len() {
        push_change(
            &mut regions,
            (nb.len(), nb.len()),
            (li, nl.len()),
            (ri, nr.len()),
        );
    }

    regions
}

/// Coalesce single-char changed positions into `[start, end)` spans.
fn coalesce(cols: &[usize]) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for &c in cols {
        match spans.last_mut() {
            Some(last) if last.1 == c => last.1 = c + 1,
            _ => spans.push((c, c + 1)),
        }
    }
    spans
}

/// A `[start_col, end_col)` char range within a line.
pub type ColSpan = (usize, usize);

/// Per-line word/character spans that differ between `a` and `b`.
/// Returns `(spans_in_a, spans_in_b)`.
pub fn word_spans(a: &str, b: &str) -> (Vec<ColSpan>, Vec<ColSpan>) {
    let diff = TextDiff::from_chars(a, b);
    let mut a_cols = Vec::new();
    let mut b_cols = Vec::new();
    let (mut ai, mut bi) = (0usize, 0usize);
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                ai += 1;
                bi += 1;
            }
            ChangeTag::Delete => {
                a_cols.push(ai);
                ai += 1;
            }
            ChangeTag::Insert => {
                b_cols.push(bi);
                bi += 1;
            }
        }
    }
    (coalesce(&a_cols), coalesce(&b_cols))
}
