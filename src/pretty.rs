const BYTE_UNITS: &[&'static str] = &[
    "b",
    "kb",
    "mb",
    "gb",
    "tb",
    "pb",
    "eb",
    "zb",
    "yb"
];

pub fn bytes(num: usize) -> String {
    if num == 0 {
        return format!("{}{}", num, BYTE_UNITS[0])
    }
    
    let num = num as f64;
    let exp: f64 = num.log10() / 3_f64;
    let exp: f64 = exp.floor();
    let exp: f64 = exp.min((BYTE_UNITS.len() as f64) - 1_f64);
    let num = num / 1_000f64.powf(exp);
    let unit = BYTE_UNITS[exp as usize];

    format!("{:.2}{}", num, unit)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn pretty_bytes() {
        let params: Vec<(u64, &str)> = vec! {
            (0,  "0b"),
            (1,  "1.00b"),
            (10, "10.00b"),
            (999, "999.00b"),
            (1001, "1.00kb"),
            (10_u64.pow(3) + 12456, "13.46kb"),
            (10_u64.pow(16), "10.00pb"),
        };
        
        for (num, expected) in params {
            let actual = bytes(num as usize);
            println!("'{}' '{}'", num, expected);
            assert_eq!(expected, actual);
        }
    }
}