fn near(a: &[u8], b: &[u8], max: u8) -> bool {
    !a.is_empty() && a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.abs_diff(*b) <= max)
}

// Candidate order is the script's musical preference; feasibility is a hard
// constraint, including the seam and every old/new edge during live adoption.
pub fn choose(
    candidates: &[Vec<Vec<u8>>],
    previous: &[Vec<u8>],
    max: u8,
    pin: Option<usize>,
) -> Option<Vec<usize>> {
    let n = candidates.len();
    if n == 0
        || n > 64
        || (!previous.is_empty() && previous.len() != n)
        || pin.is_some_and(|i| i >= n)
        || candidates.iter().any(|c| c.is_empty() || c.len() > 128)
    {
        return None;
    }
    let allowed = |i: usize, notes: &[u8]| {
        !notes.is_empty()
            && notes.len() <= 8
            && notes.iter().all(|p| *p <= 127)
            && notes.windows(2).all(|p| p[0] < p[1])
            && (previous.is_empty()
                || (near(notes, &previous[(i + n - 1) % n], max)
                    && near(notes, &previous[(i + 1) % n], max)
                    && (pin != Some(i) || notes == previous[i])))
    };
    let mut best: Option<(usize, Vec<usize>)> = None;
    for (first, start) in candidates[0].iter().enumerate() {
        if !allowed(0, start) {
            continue;
        }
        let mut paths = vec![None; candidates[0].len()];
        paths[first] = Some((first, vec![first]));
        for i in 1..n {
            let mut next = vec![None; candidates[i].len()];
            for (j, notes) in candidates[i].iter().enumerate() {
                if !allowed(i, notes) {
                    continue;
                }
                for (k, path) in paths.iter().enumerate() {
                    let Some((cost, path)) = path else {
                        continue;
                    };
                    if !near(&candidates[i - 1][k], notes, max) {
                        continue;
                    }
                    let score = cost + j;
                    if next[j].as_ref().is_none_or(|(old, _)| score < *old) {
                        let mut path = path.clone();
                        path.push(j);
                        next[j] = Some((score, path));
                    }
                }
            }
            paths = next;
        }
        for (last, path) in paths.into_iter().enumerate() {
            let Some((cost, path)) = path else {
                continue;
            };
            if near(&candidates[n - 1][last], start, max)
                && best.as_ref().is_none_or(|(old, _)| cost < *old)
            {
                best = Some((cost, path));
            }
        }
    }
    best.map(|(_, path)| path)
}
