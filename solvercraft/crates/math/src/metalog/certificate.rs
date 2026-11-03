//! Global certificate using outward-rounded interval arithmetic.
const CUT: f64 = 1.0 / 65536.0;
const MAX_DEPTH: u8 = 24;
const MAX_INTERVALS: usize = 8192;

#[derive(Clone, Copy)]
struct Interval {
    lo: f64,
    hi: f64,
}

impl Interval {
    fn point(value: f64) -> Self {
        Self { lo: value, hi: value }
    }

    fn add(self, rhs: Self) -> Self {
        Self { lo: (self.lo + rhs.lo).next_down(), hi: (self.hi + rhs.hi).next_up() }
    }

    fn sub(self, rhs: Self) -> Self {
        self.add(Self { lo: -rhs.hi, hi: -rhs.lo })
    }

    fn mul(self, rhs: Self) -> Self {
        let products = [self.lo * rhs.lo, self.lo * rhs.hi, self.hi * rhs.lo, self.hi * rhs.hi];
        Self {
            lo: products.into_iter().fold(f64::INFINITY, f64::min).next_down(),
            hi: products.into_iter().fold(f64::NEG_INFINITY, f64::max).next_up(),
        }
    }

    fn magnitude(self) -> f64 {
        self.lo.abs().max(self.hi.abs())
    }
}

/// Enclose ln(x) on [1,2] without relying on the platform libm's rounding.
/// At x=2 the atanh series ratio is 1/3. After 24 terms its omitted tail
/// is at most 2*3^-49/(49*(1-1/9)), strictly below 2^-70.
fn log_unit(x: f64) -> Interval {
    let a = Interval::point(x).sub(Interval::point(1.0));
    let b = Interval::point(x).add(Interval::point(1.0));
    let y = Interval { lo: (a.lo.max(0.0) / b.hi).next_down().max(0.0), hi: (a.hi / b.lo).next_up() };
    let y2 = y.mul(y);
    let mut power = y;
    let mut sum = Interval::point(0.0);
    for index in 0..24 {
        let divisor = f64::from(2 * index + 1);
        sum = sum.add(Interval { lo: (power.lo / divisor).next_down(), hi: (power.hi / divisor).next_up() });
        power = power.mul(y2);
    }
    let sum = sum.mul(Interval::point(2.0));
    Interval { lo: sum.lo, hi: (sum.hi + 2.0_f64.powi(-70)).next_up() }
}

/// Inputs stay in [CUT,1] so every scaling operation is exact and normal.
fn log_probability(mut p: f64, log2: Interval) -> Interval {
    let mut exponent = 0;
    while p < 1.0 {
        p *= 2.0;
        exponent -= 1;
    }
    log_unit(p).add(log2.mul(Interval::point(f64::from(exponent))))
}

fn logit(p: f64, log2: Interval) -> Interval {
    let complement = Interval::point(1.0).sub(Interval::point(p));
    let low = log_probability(complement.lo, log2).lo;
    let high = log_probability(complement.hi, log2).hi;
    log_probability(p, log2).sub(Interval { lo: low, hi: high })
}

fn polynomial(coefficients: &[Interval], x: Interval) -> Interval {
    coefficients.iter().rev().fold(Interval::point(0.0), |sum, coefficient| sum.mul(x).add(*coefficient))
}

fn derivative(coefficients: &[Interval]) -> Vec<Interval> {
    coefficients.iter().enumerate().skip(1).map(|(power, coefficient)| coefficient.mul(Interval::point(power as f64))).collect()
}

/// Write the latent quantile as A(t) + B(t)logit(p), t=p-1/2.
/// Its logit derivative is F=B+v(A'+B'logit(p)), v=p(1-p).
/// The center value and a bound on dF/dp certify an entire interval.
pub(super) fn certify(coefficients: &[f64]) -> Option<f64> {
    // The interval operations below are finite in this fallback's numerical
    // envelope. The primary dominance certificate remains available outside it.
    if coefficients.iter().skip(1).any(|a| a.abs() > 1e100) {
        return None;
    }
    let mut a = vec![Interval::point(0.0)];
    let mut b = Vec::new();
    for (index, value) in coefficients.iter().enumerate().skip(1) {
        if index == 1 || index == 2 || (index >= 5 && index % 2 == 1) {
            b.push(Interval::point(*value));
        } else {
            a.push(Interval::point(*value));
        }
    }
    let da = derivative(&a);
    let db = derivative(&b);
    let dda = derivative(&da);
    let ddb = derivative(&db);
    let half = Interval::point(0.5);
    let one = Interval::point(1.0);
    let two = Interval::point(2.0);
    // In either tail v <= CUT and |logit(p)|v <= CUT*log(1/CUT) < 16*CUT.
    let mut margin = f64::INFINITY;
    for t in [Interval { lo: -0.5, hi: CUT - 0.5 }, Interval { lo: 0.5 - CUT, hi: 0.5 }] {
        let penalty = Interval::point(CUT)
            .mul(Interval::point(polynomial(&da, t).magnitude()))
            .add(Interval::point(16.0 * CUT).mul(Interval::point(polynomial(&db, t).magnitude())));
        let lower = polynomial(&b, t).sub(penalty).lo;
        if lower <= 0.0 {
            return None;
        }
        margin = margin.min(lower);
    }
    let log2 = log_unit(2.0);
    let mut pending = vec![(CUT, 1.0 - CUT, logit(CUT, log2), logit(1.0 - CUT, log2), 0)];
    for _ in 0..MAX_INTERVALS {
        let Some((lo, hi, zl, zh, depth)) = pending.pop() else { return Some(margin) };
        let center = 0.5 * lo + 0.5 * hi;
        let zc = logit(center, log2);
        let c = Interval::point(center);
        let tc = c.sub(half);
        let fc = polynomial(&b, tc).add(c.mul(one.sub(c)).mul(polynomial(&da, tc).add(polynomial(&db, tc).mul(zc))));
        if fc.hi <= 0.0 {
            return None;
        }
        let p = Interval { lo, hi };
        let t = p.sub(half);
        let z = Interval { lo: zl.lo, hi: zh.hi };
        let first = polynomial(&db, t)
            .mul(two)
            .add(one.sub(p.mul(two)).mul(polynomial(&da, t).add(polynomial(&db, t).mul(z))))
            .add(p.mul(one.sub(p)).mul(polynomial(&dda, t).add(polynomial(&ddb, t).mul(z))));
        let radius = c.sub(Interval::point(lo)).hi.max(Interval::point(hi).sub(c).hi);
        let lower = fc.sub(Interval::point(first.magnitude()).mul(Interval::point(radius))).lo;
        if lower > 0.0 {
            margin = margin.min(lower);
        } else {
            if depth >= MAX_DEPTH || !(lo < center && center < hi) {
                return None;
            }
            pending.push((center, hi, zc, zh, depth + 1));
            pending.push((lo, center, zl, zc, depth + 1));
        }
    }
    None
}
