//! Small JSON writers. A JSON library costs far more wasm fuel than a callback allows.

pub fn push_u(out: &mut String, n: u64) {
    if n < 10 {
        out.push((b'0' + n as u8) as char);
        return;
    }
    if n <= u64::from(u32::MAX) {
        return push_u32(out, n as u32, false);
    }
    let mut buf = [0u8; 20];
    let mut index = buf.len();
    let mut n = n;
    while n > 0 {
        index -= 1;
        buf[index] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    push_digits(out, &buf[index..]);
}

/// Appends ASCII digits (and a sign) without re-validating them as UTF-8.
fn push_digits(out: &mut String, digits: &[u8]) {
    // SAFETY: callers only pass ASCII digits and '-', which are valid UTF-8.
    out.push_str(unsafe { std::str::from_utf8_unchecked(digits) });
}

/// 32-bit formatting: noticeably cheaper in wasm fuel than 64-bit division.
pub fn push_u32(out: &mut String, mut n: u32, negative: bool) {
    let mut buf = [0u8; 11];
    let mut index = buf.len();
    loop {
        index -= 1;
        buf[index] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if negative {
        index -= 1;
        buf[index] = b'-';
    }
    push_digits(out, &buf[index..]);
}

/// A non-negative number with up to three decimals.
pub fn num(out: &mut String, value: f64) {
    let x = if value.is_finite() && value > 0.0 { value } else { 0.0 };
    let scaled = (x * 1000.0 + 0.5) as u64;
    push_u(out, scaled / 1000);
    let mut fraction = scaled % 1000;
    if fraction != 0 {
        out.push('.');
        let mut divisor = 100;
        while fraction != 0 {
            out.push((b'0' + (fraction / divisor) as u8) as char);
            fraction %= divisor;
            divisor /= 10;
        }
    }
}

/// A quoted string; callers only pass labels and key names, which need no escaping.
pub fn q(out: &mut String, text: &str) {
    out.push('"');
    out.push_str(text);
    out.push('"');
}

pub fn rgba(out: &mut String, color: [f32; 4]) {
    out.push('[');
    for (index, channel) in color.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        num(out, f64::from(*channel));
    }
    out.push(']');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_colours_are_plain_json() {
        let mut out = String::new();
        num(&mut out, 0.5);
        out.push(' ');
        num(&mut out, 12.0);
        out.push(' ');
        rgba(&mut out, [1.0, 0.0, 0.33, 0.5]);
        assert_eq!(out, "0.5 12 [1,0,0.33,0.5]");
        let mut out = String::new();
        push_i(&mut out, -42);
        assert_eq!(out, "-42");
    }
}

/// A signed integer.
pub fn push_i(out: &mut String, n: i64) {
    if let Ok(small) = i32::try_from(n) {
        return push_u32(out, small.unsigned_abs(), small < 0);
    }
    if n < 0 {
        out.push('-');
    }
    push_u(out, n.unsigned_abs());
}



/// serde_json's compact JSON, with a shortcut for the floats the settings are made of.
///
/// Writing a float the general way (the shortest text that reads back as the same number) costs
/// several hundred fuel in wasm, and the settings hold hundreds of them, nearly all with at most two
/// decimals (sizes, offsets, opacities). Those are written directly; every other number goes the
/// general way. Either way the text is exactly what `serde_json::to_string` writes (checked by the
/// tests below), so the settings file does not change.
pub fn to_json<T: serde::Serialize + ?Sized>(value: &T) -> serde_json::Result<String> {
    let mut out = Vec::with_capacity(16 * 1024);
    value.serialize(&mut serde_json::Serializer::with_formatter(&mut out, ShortFloats))?;
    // SAFETY: serde_json writes only valid UTF-8, and `short` only ASCII digits, '-' and '.'.
    Ok(unsafe { String::from_utf8_unchecked(out) })
}

struct ShortFloats;

impl serde_json::ser::Formatter for ShortFloats {
    fn write_f64<W: ?Sized + std::io::Write>(&mut self, writer: &mut W, value: f64) -> std::io::Result<()> {
        let mut buffer = [0u8; 24];
        match short(value, |a| a < 1e9, |k| k as f64 / 100.0 == value.abs(), &mut buffer) {
            Some(length) => writer.write_all(&buffer[..length]),
            None => serde_json::ser::CompactFormatter.write_f64(writer, value),
        }
    }

    fn write_f32<W: ?Sized + std::io::Write>(&mut self, writer: &mut W, value: f32) -> std::io::Result<()> {
        let mut buffer = [0u8; 24];
        let exact = value.abs();
        // The two decimals read back as this f32 when they are well inside its rounding interval
        // (a quarter of the gap to the next f32), so no double rounding can tip them over.
        let reads_back = |k: u64| {
            let decimal = k as f64 / 100.0;
            let gap = f64::from(f32::from_bits(exact.to_bits() + 1)) - f64::from(exact);
            (decimal - f64::from(exact)).abs() <= gap / 4.0
        };
        match short(f64::from(value), |a| a < 1e5, reads_back, &mut buffer) {
            Some(length) => writer.write_all(&buffer[..length]),
            None => serde_json::ser::CompactFormatter.write_f32(writer, value),
        }
    }
}

/// `value` as the shortest text that reads back as it, when that text has at most two decimals and
/// `value` is in the range `small_enough` allows; none otherwise. Within that range two different
/// numbers with at most two decimals are further apart than one step of the float, so a two-decimal
/// text that reads back is the shortest one, and it is written the way serde_json writes it ("12.0",
/// "0.25", "-3.5").
fn short(value: f64, small_enough: impl Fn(f64) -> bool, reads_back: impl Fn(u64) -> bool, out: &mut [u8; 24]) -> Option<usize> {
    let negative = value.is_sign_negative();
    let a = value.abs();
    if a == 0.0 {
        if negative {
            return None;
        }
        out[..3].copy_from_slice(b"0.0");
        return Some(3);
    }
    if !(a >= 0.01 && small_enough(a)) {
        return None;
    }
    let k = (a * 100.0).round() as u64;
    if !reads_back(k) {
        return None;
    }
    let mut at = 0;
    if negative {
        out[0] = b'-';
        at = 1;
    }
    let whole = k / 100;
    let mut digits = [0u8; 20];
    let mut start = digits.len();
    let mut n = whole;
    loop {
        start -= 1;
        digits[start] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let length = digits.len() - start;
    out[at..at + length].copy_from_slice(&digits[start..]);
    at += length;
    out[at] = b'.';
    at += 1;
    let fraction = k % 100;
    out[at] = b'0' + (fraction / 10) as u8;
    at += 1;
    if fraction % 10 != 0 {
        out[at] = b'0' + (fraction % 10) as u8;
        at += 1;
    }
    Some(at)
}

#[cfg(test)]
mod short_float_tests {
    use super::to_json;

    fn same<T: serde::Serialize + ?Sized>(value: &T) {
        assert_eq!(to_json(value).unwrap(), serde_json::to_string(value).unwrap());
    }

    #[test]
    fn floats_are_written_as_serde_json_writes_them() {
        let mut doubles: Vec<f64> = Vec::new();
        let mut singles: Vec<f32> = Vec::new();
        for k in -2_000_000i64..=2_000_000 {
            doubles.push(k as f64 / 100.0);
            singles.push((k as f64 / 100.0) as f32);
            singles.push(k as f32 / 100.0);
        }
        for k in (0..4_000_000_000_000u64).step_by(9_999_991) {
            doubles.push(k as f64 / 100.0);
            doubles.push(-(k as f64) / 1000.0);
        }
        for special in [0.0, -0.0, 1e-7, 1e-5, 0.001, 0.009, 0.0099999, 0.01, 0.015, 0.1, 0.3, 0.30000000000000004, 1e9, 999999999.99, 1e15, 1e16, 1e17, 1e21, f64::MAX, f64::MIN_POSITIVE, 5e-324, 2.5, 0.125, 1.005, 2.675] {
            doubles.push(special);
            doubles.push(-special);
            singles.push(special as f32);
            singles.push(-special as f32);
        }
        for special in [99999.99f32, 100000.0, 131071.99, 131072.0, 0.33, 16777216.0, 1.1, 2.2, 3.3] {
            singles.push(special);
            singles.push(-special);
        }
        // Arbitrary bit patterns, finite ones only (serde_json writes the others as null).
        let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
        for _ in 0..300_000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let d = f64::from_bits(seed);
            if d.is_finite() {
                doubles.push(d);
            }
            let s = f32::from_bits(seed as u32);
            if s.is_finite() {
                singles.push(s);
            }
        }
        for chunk in doubles.chunks(10_000) {
            same(chunk);
        }
        for chunk in singles.chunks(10_000) {
            same(chunk);
        }
        same(&[f64::NAN, f64::INFINITY, -1.5]);
        same(&[f32::NAN, f32::NEG_INFINITY, 0.25]);
    }

    #[test]
    fn settings_are_written_as_serde_json_writes_them() {
        use crate::config::{Element, Settings};
        let mut settings = Settings::default();
        same(&settings);
        for (n, element) in Element::ALL.into_iter().enumerate() {
            let style = settings.style_mut(element);
            style.enabled = true;
            style.scale = 0.25 + n as f64 * 0.37;
            style.opacity = 100.0 / (n as f64 + 3.0);
            style.position = Some([n as f32 * 0.0123, 1.0 - n as f32 * 0.031]);
        }
        same(&settings);
    }
}
