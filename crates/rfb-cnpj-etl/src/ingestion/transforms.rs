//! In-flight text transcoding, BRL currency parsing, and CNPJ synthesis.
//!
//! Designed for zero-copy streaming operation over Windows-1252 encoded RFB CNPJ open data.

use std::borrow::Cow;
use std::io::Read;

/// Trims ASCII whitespace (`' '`, `'\t'`, `'\r'`, `'\n'`) from both ends of a byte slice.
#[inline]
pub fn trim_ascii_whitespace(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|&b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|&b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        .map(|pos| pos + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

/// Decodes a byte slice from `WINDOWS_1252` (ISO-8859-1) to UTF-8 `Cow<'_, str>`.
///
/// If the byte slice contains only ASCII bytes, returns `Cow::Borrowed` with zero allocations.
/// If it contains non-ASCII Latin-1 characters (e.g. `Ç`, `Ã`, `É`), returns `Cow::Owned(String)`.
#[inline]
pub fn decode_windows_1252(bytes: &[u8]) -> Cow<'_, str> {
    let trimmed = trim_ascii_whitespace(bytes);
    let (cow, _encoding, _had_errors) = encoding_rs::WINDOWS_1252.decode(trimmed);
    cow
}

/// Decodes a byte slice from `WINDOWS_1252` to an owned `String`, trimming leading/trailing whitespace.
#[inline]
pub fn decode_trimmed_str(bytes: &[u8]) -> String {
    decode_windows_1252(bytes).into_owned()
}

/// Decodes an optional field from `WINDOWS_1252`. Returns `None` if empty after trimming.
#[inline]
pub fn decode_opt_windows_1252(bytes: &[u8]) -> Option<String> {
    let cow = decode_windows_1252(bytes);
    if cow.is_empty() {
        None
    } else {
        Some(cow.into_owned())
    }
}

/// Parses Brazilian currency/float strings into `f64` in-flight with zero heap allocations.
///
/// Handles official RFB formats:
/// - `"1500,50"` -> `1500.50`
/// - `"1.500,50"` -> `1500.50`
/// - `"0,00"` or `""` or `" "` -> `0.0`
/// - `"-250,75"` -> `-250.75`
/// - `"1500.50"` (standard float fallback) -> `1500.50`
#[inline]
pub fn parse_brl_float(bytes: &[u8]) -> f64 {
    let trimmed = trim_ascii_whitespace(bytes);
    if trimmed.is_empty() {
        return 0.0;
    }

    // Stack buffer for in-place ASCII normalization (up to 64 bytes)
    let mut buf = [0u8; 64];
    if trimmed.len() >= buf.len() {
        let s = String::from_utf8_lossy(trimmed);
        return parse_brl_float_str(&s);
    }

    let has_comma = trimmed.contains(&b',');
    let dot_count = trimmed.iter().filter(|&&b| b == b'.').count();

    let mut j = 0;
    for &b in trimmed {
        if b == b',' {
            buf[j] = b'.';
            j += 1;
        } else if b == b'.' {
            if has_comma || dot_count > 1 {
                // Dot is a thousands separator: skip it!
                continue;
            } else {
                // Single dot without comma: decimal point
                buf[j] = b'.';
                j += 1;
            }
        } else if b.is_ascii_digit() || b == b'-' || b == b'+' {
            buf[j] = b;
            j += 1;
        }
    }

    if j == 0 {
        return 0.0;
    }

    std::str::from_utf8(&buf[..j])
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// Fallback helper for unusually long string numbers.
fn parse_brl_float_str(raw: &str) -> f64 {
    let raw = raw.trim();
    if raw.is_empty() {
        return 0.0;
    }
    let has_dot = raw.contains('.');
    let has_comma = raw.contains(',');

    let normalized: String = if has_dot && has_comma {
        raw.chars()
            .filter(|&c| c != '.')
            .map(|c| if c == ',' { '.' } else { c })
            .collect()
    } else if has_comma {
        raw.chars()
            .map(|c| if c == ',' { '.' } else { c })
            .collect()
    } else {
        raw.to_string()
    };

    normalized.parse::<f64>().unwrap_or(0.0)
}

/// Parses an optional Brazilian currency string into `Option<f64>`. Returns `None` if empty.
#[inline]
pub fn parse_brl_float_opt(bytes: &[u8]) -> Option<f64> {
    let trimmed = trim_ascii_whitespace(bytes);
    if trimmed.is_empty() {
        None
    } else {
        Some(parse_brl_float(trimmed))
    }
}

/// Synthesizes a canonical 14-digit CNPJ string from `(cnpj_basico, cnpj_ordem, cnpj_dv)`.
///
/// - `cnpj_basico`: 8 digits (radical)
/// - `cnpj_ordem`: 4 digits (filial)
/// - `cnpj_dv`: 2 digits (dígitos verificadores)
///
/// Guarantees exact 14 characters by left-padding with `'0'` if raw fields are shorter.
#[inline]
pub fn synthesize_cnpj(basico: &[u8], ordem: &[u8], dv: &[u8]) -> String {
    let bytes = synthesize_cnpj_bytes(basico, ordem, dv);
    std::str::from_utf8(&bytes)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "00000000000000".to_string())
}

/// Synthesizes a canonical 14-digit CNPJ into a fixed stack array `[u8; 14]` with zero allocations.
#[inline]
pub fn synthesize_cnpj_bytes(basico: &[u8], ordem: &[u8], dv: &[u8]) -> [u8; 14] {
    let mut out = [b'0'; 14];
    pad_copy_right_aligned(trim_ascii_whitespace(basico), &mut out[0..8]);
    pad_copy_right_aligned(trim_ascii_whitespace(ordem), &mut out[8..12]);
    pad_copy_right_aligned(trim_ascii_whitespace(dv), &mut out[12..14]);
    out
}

#[inline]
fn pad_copy_right_aligned(src: &[u8], dest: &mut [u8]) {
    let dest_len = dest.len();
    let src_len = src.len().min(dest_len);
    let offset = dest_len - src_len;
    dest[..offset].fill(b'0');
    dest[offset..].copy_from_slice(&src[src.len() - src_len..]);
}

/// Extracts reference date and year-month from an RFB inner zip entry filename.
///
/// Typical RFB entry filenames:
/// - `K3241.K03200Y0.D30610.EMPRECSV` -> `("10/06/2023", "202306")`
/// - `F.K03200$Z.D60810.ESTABELE` -> `("10/08/2026", "202608")`
/// - `F.K03200$Z.D61024.ESTABELE` -> `("24/10/2026", "202610")`
///
/// Format: `D` + `Y` (last digit of year 202Y) + `MM` (month) + `DD` (day).
pub fn parse_rfb_reference_code(entry_name: &str) -> Option<(String, String)> {
    for part in entry_name.split('.') {
        if !part.starts_with('D') {
            continue;
        }

        let bytes = part.as_bytes();

        // 6-character format: D + Y + MM + DD (e.g. D61024 -> 24/10/2026, 202610)
        if bytes.len() == 6 && bytes[1..].iter().all(|b| b.is_ascii_digit()) {
            let year_digit = bytes[1] as char;
            let month = &part[2..4];
            let day = &part[4..6];

            if let (Ok(m), Ok(d)) = (month.parse::<u32>(), day.parse::<u32>()) {
                if (1..=12).contains(&m) && (1..=31).contains(&d) {
                    let full_year = format!("202{}", year_digit);
                    let data_referencia = format!("{}/{}/{}", day, month, full_year);
                    let ano_mes = format!("{}{}", full_year, month);
                    return Some((data_referencia, ano_mes));
                }
            }
        // 7-character format: D + YY + MM + DD (e.g. D261024 -> 24/10/2026, 202610)
        } else if bytes.len() == 7 && bytes[1..].iter().all(|b| b.is_ascii_digit()) {
            let year_digits = &part[1..3];
            let month = &part[3..5];
            let day = &part[5..7];

            if let (Ok(m), Ok(d)) = (month.parse::<u32>(), day.parse::<u32>()) {
                if (1..=12).contains(&m) && (1..=31).contains(&d) {
                    let full_year = format!("20{}", year_digits);
                    let data_referencia = format!("{}/{}/{}", day, month, full_year);
                    let ano_mes = format!("{}{}", full_year, month);
                    return Some((data_referencia, ano_mes));
                }
            }
        }
    }
    None
}

/// Streaming reader that on-the-fly transcodes `WINDOWS_1252` bytes to UTF-8.
///
/// Wraps any `R: std::io::Read` and implements `std::io::Read`, producing valid UTF-8.
pub struct Windows1252Reader<R> {
    inner: R,
    decoder: encoding_rs::Decoder,
    in_buf: [u8; 8192],
    in_len: usize,
    in_pos: usize,
    out_buf: [u8; 8192],
    out_len: usize,
    out_pos: usize,
    eof_reached: bool,
}

impl<R: Read> Windows1252Reader<R> {
    /// Creates a new `Windows1252Reader` wrapping the given stream.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            decoder: encoding_rs::WINDOWS_1252.new_decoder(),
            in_buf: [0u8; 8192],
            in_len: 0,
            in_pos: 0,
            out_buf: [0u8; 8192],
            out_len: 0,
            out_pos: 0,
            eof_reached: false,
        }
    }
}

impl<R: Read> Read for Windows1252Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            // 1. Drain pending UTF-8 bytes from out_buf
            if self.out_pos < self.out_len {
                let avail = self.out_len - self.out_pos;
                let to_copy = avail.min(buf.len());
                buf[..to_copy].copy_from_slice(&self.out_buf[self.out_pos..self.out_pos + to_copy]);
                self.out_pos += to_copy;
                return Ok(to_copy);
            }

            // Reset out buffer pointers
            self.out_pos = 0;
            self.out_len = 0;

            // 2. If in_buf is empty, read more bytes from inner
            if self.in_pos >= self.in_len {
                self.in_pos = 0;
                self.in_len = self.inner.read(&mut self.in_buf)?;
                if self.in_len == 0 {
                    self.eof_reached = true;
                }
            }

            // 3. If EOF reached and nothing to decode, return 0
            if self.eof_reached && self.in_len == 0 {
                return Ok(0);
            }

            // 4. Decode in_buf into out_buf
            let (result, read_bytes, written_bytes, _had_errors) = self.decoder.decode_to_utf8(
                &self.in_buf[self.in_pos..self.in_len],
                &mut self.out_buf,
                self.eof_reached,
            );

            self.in_pos += read_bytes;
            self.out_len = written_bytes;

            if written_bytes > 0 {
                let to_copy = written_bytes.min(buf.len());
                buf[..to_copy].copy_from_slice(&self.out_buf[..to_copy]);
                self.out_pos = to_copy;
                return Ok(to_copy);
            }

            if let encoding_rs::CoderResult::InputEmpty = result {
                if self.eof_reached {
                    return Ok(0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_trim_ascii_whitespace() {
        assert_eq!(trim_ascii_whitespace(b"  abc  "), b"abc");
        assert_eq!(trim_ascii_whitespace(b"\r\n\t"), b"");
        assert_eq!(trim_ascii_whitespace(b"hello"), b"hello");
    }

    #[test]
    fn test_decode_windows_1252_ascii() {
        let input = b"PETROBRAS SA";
        let res = decode_windows_1252(input);
        assert_eq!(res, "PETROBRAS SA");
        assert!(matches!(res, Cow::Borrowed(_)));
    }

    #[test]
    fn test_decode_windows_1252_accented() {
        // Windows-1252 bytes for "COMÉRCIO & PARTICIPAÇÕES"
        let input = &[
            b'C', b'O', b'M', 0xC9, b'R', b'C', b'I', b'O', b' ', b'&', b' ', b'P', b'A', b'R',
            b'T', b'I', b'C', b'I', b'P', b'A', 0xC7, 0xD5, b'E', b'S',
        ];
        let res = decode_windows_1252(input);
        assert_eq!(res, "COMÉRCIO & PARTICIPAÇÕES");
        assert!(matches!(res, Cow::Owned(_)));
    }

    #[test]
    fn test_parse_brl_float() {
        assert_eq!(parse_brl_float(b"1500,50"), 1500.50);
        assert_eq!(parse_brl_float(b"1.500,50"), 1500.50);
        assert_eq!(parse_brl_float(b"1.234.567,89"), 1234567.89);
        assert_eq!(parse_brl_float(b"0,00"), 0.0);
        assert_eq!(parse_brl_float(b""), 0.0);
        assert_eq!(parse_brl_float(b"   "), 0.0);
        assert_eq!(parse_brl_float(b"-250,75"), -250.75);
        assert_eq!(parse_brl_float(b"500"), 500.0);
        assert_eq!(parse_brl_float(b"1500.50"), 1500.50);
    }

    #[test]
    fn test_synthesize_cnpj() {
        assert_eq!(
            synthesize_cnpj(b"00000000", b"0001", b"91"),
            "00000000000191"
        );
        assert_eq!(synthesize_cnpj(b"33000167", b"1", b"1"), "33000167000101");
        assert_eq!(
            synthesize_cnpj(b"  12345678  ", b" 0002 ", b" 99 "),
            "12345678000299"
        );
    }

    #[test]
    fn test_parse_rfb_reference_code() {
        let (dt, ym) = parse_rfb_reference_code("K3241.K03200Y0.D30610.EMPRECSV").unwrap();
        assert_eq!(dt, "10/06/2023");
        assert_eq!(ym, "202306");

        let (dt2, ym2) = parse_rfb_reference_code("F.K03200$Z.D60810.ESTABELE").unwrap();
        assert_eq!(dt2, "10/08/2026");
        assert_eq!(ym2, "202608");

        let (dt3, ym3) = parse_rfb_reference_code("F.K03200$Z.D61024.SOCIOCSV").unwrap();
        assert_eq!(dt3, "24/10/2026");
        assert_eq!(ym3, "202610");

        // 7-character format
        let (dt4, ym4) = parse_rfb_reference_code("F.K03200$Z.D261024.SOCIOCSV").unwrap();
        assert_eq!(dt4, "24/10/2026");
        assert_eq!(ym4, "202610");

        // Non-digit alphabetic tokens starting with 'D' must be rejected
        assert_eq!(parse_rfb_reference_code("K3241.DELETE.EMPRECSV"), None);
        assert_eq!(parse_rfb_reference_code("K3241.DEFAULT.EMPRECSV"), None);
        assert_eq!(parse_rfb_reference_code("K3241.DEBUG0.EMPRECSV"), None);
        assert_eq!(parse_rfb_reference_code("K3241.DOMAIN.EMPRECSV"), None);

        // Out-of-bounds calendar values must be rejected
        assert_eq!(parse_rfb_reference_code("K3241.D60000.EMPRECSV"), None);
        assert_eq!(parse_rfb_reference_code("K3241.D61301.EMPRECSV"), None);
        assert_eq!(parse_rfb_reference_code("K3241.D60532.EMPRECSV"), None);

        // Masking resilience: dotted words starting with D preceding a valid date
        assert_eq!(
            parse_rfb_reference_code("K3241.DELETE.D61024.EMPRECSV"),
            Some(("24/10/2026".to_string(), "202610".to_string()))
        );
        assert_eq!(
            parse_rfb_reference_code("K3241.DEFAULT.D261024.EMPRECSV"),
            Some(("24/10/2026".to_string(), "202610".to_string()))
        );
    }

    #[test]
    fn test_windows1252_reader_streaming() {
        // "SÃO PAULO" in Windows-1252: 'Ã' is 0xC3
        let win_bytes = vec![b'S', 0xC3, b'O', b' ', b'P', b'A', b'U', b'L', b'O'];
        let cursor = Cursor::new(win_bytes);
        let mut reader = Windows1252Reader::new(cursor);
        let mut utf8_str = String::new();
        reader.read_to_string(&mut utf8_str).unwrap();
        assert_eq!(utf8_str, "SÃO PAULO");
    }
}
