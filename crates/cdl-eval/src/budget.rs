//! 評価ステップの予算（SPEC 第 7 章、DECISIONS D4）。

/// 予算を使い切った。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exhausted;

/// 残りのステップ数。減るだけで、増えない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    remaining: u64,
}

impl Budget {
    pub fn new(steps: u64) -> Self {
        Budget { remaining: steps }
    }

    pub fn remaining(&self) -> u64 {
        self.remaining
    }

    /// `n` ステップを使う。足りなければ残りを 0 にして [`Exhausted`] を返す。
    pub fn charge(&mut self, n: u64) -> Result<(), Exhausted> {
        match self.remaining.checked_sub(n) {
            Some(rest) => {
                self.remaining = rest;
                Ok(())
            }
            None => {
                self.remaining = 0;
                Err(Exhausted)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charges_until_exhausted() {
        let mut b = Budget::new(3);
        assert_eq!(b.charge(2), Ok(()));
        assert_eq!(b.remaining(), 1);
        assert_eq!(b.charge(2), Err(Exhausted));
        assert_eq!(b.remaining(), 0);
        assert_eq!(b.charge(1), Err(Exhausted));
    }
}

/// Kani のハーネス（DECISIONS F5: K5）。
#[cfg(kani)]
mod verification {
    use super::*;

    /// K5: `charge(n)` は、成功すれば残りを厳密に `n` だけ減らし、失敗すれば残りを 0 にする。
    #[kani::proof]
    fn charge_is_monotone() {
        let start: u64 = kani::any();
        let n: u64 = kani::any();
        let mut b = Budget::new(start);
        match b.charge(n) {
            Ok(()) => assert!(b.remaining() == start - n),
            Err(Exhausted) => assert!(b.remaining() == 0 && n > start),
        }
        assert!(b.remaining() <= start);
    }

    /// K5: 1 以上のステップを使い続けると、予算の数 + 1 回以内に必ず止まる。
    #[kani::proof]
    #[kani::unwind(10)]
    fn charging_always_stops() {
        let start: u64 = kani::any();
        kani::assume(start < 8);
        let mut b = Budget::new(start);
        let mut calls: u64 = 0;
        loop {
            let n: u64 = kani::any();
            kani::assume(n >= 1);
            calls += 1;
            if b.charge(n).is_err() {
                break;
            }
        }
        assert!(calls <= start + 1);
    }
}
