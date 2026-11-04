//! 数値の変換と、checked な演算（SPEC 第 7 章、DECISIONS B11・D1・D2）。
//!
//! `as` による変換は使わない（DECISIONS F3）。

/// f64 が正確に表せる整数の上限（2^53）。Duration のミリ秒はこの範囲に限る。
pub const MAX_SAFE: i64 = 9_007_199_254_740_992;

/// -0 を +0 にそろえる（DECISIONS D2）。
pub fn normalize(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x }
}

/// 整数で、絶対値が 2^53 以下の f64 を i64 に変換する。それ以外は `None`。
pub fn f64_to_i64(x: f64) -> Option<i64> {
    if !x.is_finite() || x.fract() != 0.0 || x.abs() > 9_007_199_254_740_992.0 {
        return None;
    }
    let bits = x.to_bits();
    let negative = bits >> 63 == 1;
    let exponent = (bits >> 52) & 0x7ff;
    if exponent == 0 {
        // 0（非正規化数は整数でないので、ここに来るのは 0 だけ）。
        return Some(0);
    }
    let mantissa = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
    // 値 = mantissa × 2^(exponent - 1075)。整数で 2^53 以下なので、右シフトか小さな左シフトになる。
    let magnitude = if exponent >= 1075 {
        mantissa.checked_shl(u32::try_from(exponent.checked_sub(1075)?).ok()?)?
    } else {
        mantissa.checked_shr(u32::try_from(1075u64.checked_sub(exponent)?).ok()?)?
    };
    let v = i64::try_from(magnitude).ok()?;
    if negative { v.checked_neg() } else { Some(v) }
}

/// 絶対値が 2^53 以下の i64 を、正確に f64 にする。それ以外は `None`。
pub fn i64_to_f64(x: i64) -> Option<f64> {
    if x.checked_abs()? > MAX_SAFE {
        return None;
    }
    // 2^26 で分ければ、上位も下位も i32 に収まり、和も正確に表せる。
    let hi = i32::try_from(x.checked_div(1 << 26)?).ok()?;
    let lo = i32::try_from(x.checked_rem(1 << 26)?).ok()?;
    Some(f64::from(hi) * 67_108_864.0 + f64::from(lo))
}

/// JS の `Number.prototype.toString()` と同じ規則で、有限の数を文字列にする（DECISIONS D1）。
pub fn js_number_string(x: f64) -> String {
    let x = normalize(x);
    if x == 0.0 {
        return "0".to_owned();
    }
    let negative = x < 0.0;
    // `{:e}` は最短で元の値に戻る桁を出す（JS と同じ）。例: "1.5e-7"
    let sci = format!("{:e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let e: i64 = exp.parse().unwrap_or(0);
    let k = i64::try_from(digits.len()).unwrap_or(i64::MAX);
    let n = e.saturating_add(1);
    let zeros = |count: i64| "0".repeat(usize::try_from(count).unwrap_or(0));
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", zeros(n.saturating_sub(k)))
    } else if 0 < n && n <= 21 {
        let split = usize::try_from(n).unwrap_or(0);
        let (int, frac) = digits.split_at(split.min(digits.len()));
        format!("{int}.{frac}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", zeros(n.saturating_neg()))
    } else {
        let (first, rest) = digits.split_at(1.min(digits.len()));
        let e = n.saturating_sub(1);
        let sign = if e < 0 { "-" } else { "+" };
        if rest.is_empty() {
            format!("{first}e{sign}{}", e.unsigned_abs())
        } else {
            format!("{first}.{rest}e{sign}{}", e.unsigned_abs())
        }
    };
    if negative { format!("-{body}") } else { body }
}

/// Duration（整数ミリ秒）の演算。範囲は ±2^53。
pub fn duration_add(a: i64, b: i64) -> Option<i64> {
    a.checked_add(b)
        .filter(|v| v.checked_abs().is_some_and(|m| m <= MAX_SAFE))
}

pub fn duration_sub(a: i64, b: i64) -> Option<i64> {
    a.checked_sub(b)
        .filter(|v| v.checked_abs().is_some_and(|m| m <= MAX_SAFE))
}

/// 実数倍。結果が整数ミリ秒でなければ `None`。
pub fn duration_scale(ms: i64, factor: f64) -> Option<i64> {
    f64_to_i64(normalize(i64_to_f64(ms)? * factor))
}

/// 実数で割る。0 で割るときは呼び出し側で先に弾く。結果が整数ミリ秒でなければ `None`。
pub fn duration_div(ms: i64, divisor: f64) -> Option<i64> {
    f64_to_i64(normalize(i64_to_f64(ms)? / divisor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_number_strings() {
        let cases = [
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.5, "-1.5"),
            (1000.0, "1000"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (123456789012345680000.0, "123456789012345680000"),
            (1.5e-7, "1.5e-7"),
            (0.000001, "0.000001"),
            (1e-7, "1e-7"),
            (-1.0 / 3.0, "-0.3333333333333333"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
            (255.0, "255"),
            (0.5, "0.5"),
            (123.456, "123.456"),
        ];
        for (x, want) in cases {
            assert_eq!(js_number_string(x), want, "{x:e}");
        }
    }

    #[test]
    fn integer_conversions() {
        for v in [
            0i64,
            1,
            -1,
            120,
            1500,
            (1 << 40) + 3,
            -(1 << 40),
            MAX_SAFE,
            -MAX_SAFE,
            i64::from(i32::MAX) + 1,
        ] {
            let f = i64_to_f64(v).unwrap();
            assert_eq!(f64_to_i64(f), Some(v), "{v}");
        }
        assert_eq!(f64_to_i64(1.5), None);
        assert_eq!(f64_to_i64(f64::NAN), None);
        assert_eq!(f64_to_i64(1e300), None);
        assert_eq!(i64_to_f64(MAX_SAFE + 1), None);
    }

    #[test]
    fn duration_ops() {
        assert_eq!(duration_scale(250, 2.0), Some(500));
        assert_eq!(duration_scale(1, 1.5), None);
        assert_eq!(duration_div(100, 3.0), None);
        assert_eq!(duration_div(100, 4.0), Some(25));
        assert_eq!(duration_add(MAX_SAFE, 1), None);
        assert_eq!(duration_sub(-MAX_SAFE, 1), None);
    }
}

/// Kani のハーネス（DECISIONS F5: K6）。
#[cfg(kani)]
mod verification {
    use super::*;

    /// K6: Duration の加減算は、どの入力でもパニックせず、成功すれば数学的に正しい。
    #[kani::proof]
    fn duration_add_sub_are_checked() {
        let a: i64 = kani::any();
        let b: i64 = kani::any();
        if let Some(v) = duration_add(a, b) {
            assert!(i128::from(v) == i128::from(a) + i128::from(b));
            assert!(v.unsigned_abs() <= MAX_SAFE.unsigned_abs());
        }
        if let Some(v) = duration_sub(a, b) {
            assert!(i128::from(v) == i128::from(a) - i128::from(b));
        }
    }

    /// K6: f64 から i64 への変換は、どのビット列でもパニックせず、成功すれば値を保つ。
    #[kani::proof]
    fn f64_to_i64_is_exact() {
        let bits: u64 = kani::any();
        let x = f64::from_bits(bits);
        if let Some(v) = f64_to_i64(x) {
            assert!(v.unsigned_abs() <= MAX_SAFE.unsigned_abs());
            assert!(x.fract() == 0.0);
        }
    }

    /// K6: i64 から f64 への変換は、どの入力でもパニックしない。
    #[kani::proof]
    fn i64_to_f64_never_panics() {
        let v: i64 = kani::any();
        let _ = i64_to_f64(v);
    }
}
