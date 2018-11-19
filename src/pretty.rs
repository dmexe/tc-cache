const BYTE_UNITS: &[&str] = &["b", "kb", "mb", "gb", "tb", "pb", "eb", "zb", "yb"];

pub fn bytes(num: usize) -> String {
    let unit = 1024_usize;
    if num < unit {
        return format!("{}{}", num, BYTE_UNITS[0]);
    }

    let num = num as f64;
    let unit = unit as f64;
    let exp = (num.ln() / unit.ln()).floor();
    let idx = BYTE_UNITS[(exp as usize)];
    let num = num / unit.powf(exp);
    format!("{:.2}{}", num, idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_bytes() {
        let params: Vec<(u64, &str)> = vec![
            (0, "0b"),
            (1, "1b"),
            (10, "10b"),
            (999, "999b"),
            (1001, "1001b"),
            (1678, "1.64kb"),
            (14368916, "13.70mb"),
            (1186806872, "1.11gb"),
            (10_u64.pow(7) + 12456, "9.55mb"),
            (10_u64.pow(16), "8.88pb"),
        ];

        for (num, expected) in params {
            let actual = bytes(num as usize);
            assert_eq!(
                expected, actual,
                "expected {} should be {}, got {}",
                num, expected, actual
            );
        }
    }
}
