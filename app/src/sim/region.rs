//! Half-open distance intervals `[start, end)` on the course, where a skill can trigger.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub start: f64,
    pub end: f64,
}

pub const EMPTY: Region = Region {
    start: -1.0,
    end: -1.0,
};

impl Region {
    pub fn new(start: f64, end: f64) -> Self {
        Region { start, end }
    }

    /// The overlap, or `EMPTY` when there is none.
    pub fn intersect(&self, other: Region) -> Region {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        if end <= start {
            EMPTY
        } else {
            Region { start, end }
        }
    }

    pub fn fully_contains(&self, other: Region) -> bool {
        self.start <= other.start && self.end >= other.end
    }
}

/// Map every region through `f`, dropping empty results.
pub fn rmap(list: &[Region], f: impl Fn(Region) -> Vec<Region>) -> Vec<Region> {
    list.iter()
        .flat_map(|r| f(*r))
        .filter(|r| r.start > -1.0)
        .collect()
}

/// Intersect every region with `bounds`.
pub fn clip(list: &[Region], bounds: Region) -> Vec<Region> {
    rmap(list, |r| vec![r.intersect(bounds)])
}

/// Intersect every region with each of `pieces`.
pub fn clip_each(list: &[Region], pieces: &[Region]) -> Vec<Region> {
    rmap(list, |r| pieces.iter().map(|p| r.intersect(*p)).collect())
}

/// Union of two region lists, merging overlaps (same algorithm as Region.ts).
pub fn union(a: &[Region], b: &[Region]) -> Vec<Region> {
    let mut u: Vec<Region> = a.iter().chain(b).copied().collect();
    if u.is_empty() {
        return Vec::new();
    }
    u.sort_by(|x, y| x.start.total_cmp(&y.start));
    let mut out = Vec::new();
    let mut acc = u[0];
    for b in &u[1..] {
        let b = *b;
        acc = if acc.fully_contains(b) {
            acc
        } else if acc.start <= b.start && b.start < acc.end {
            Region::new(acc.start, b.end)
        } else if acc.start < b.end && b.end <= acc.end {
            Region::new(b.start, acc.end)
        } else {
            out.push(acc);
            b
        };
    }
    out.push(acc);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_merges_overlaps() {
        let a = [Region::new(0.0, 10.0), Region::new(30.0, 40.0)];
        let b = [Region::new(5.0, 20.0), Region::new(50.0, 60.0)];
        assert_eq!(
            union(&a, &b),
            vec![
                Region::new(0.0, 20.0),
                Region::new(30.0, 40.0),
                Region::new(50.0, 60.0)
            ]
        );
        assert_eq!(
            Region::new(0.0, 5.0).intersect(Region::new(5.0, 9.0)),
            EMPTY
        );
        assert_eq!(
            clip(&a, Region::new(8.0, 35.0)),
            vec![Region::new(8.0, 10.0), Region::new(30.0, 35.0)]
        );
    }
}
