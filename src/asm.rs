use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;

fn identifier(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

pub fn assemble(code: &str) -> Result<Vec<u8>> {
    const WS: fn(char) -> bool = char::is_whitespace;

    enum Val {
        Const(u8),
        Ref(String),
    }
    use Val::*;

    struct Output {
        pub mem: [Val; 256],
        pub pos: usize,
    }
    impl Output {
        pub fn new() -> Self {
            const INIT: Val = Const(0);
            Self {
                mem: [INIT; 256],
                pos: 0,
            }
        }
        pub fn push(&mut self, val: Val) -> Result<()> {
            if self.pos >= 256 {
                bail!("The compiled bytecode exceeded the limit 256");
            }
            self.mem[self.pos] = val;
            self.pos += 1;
            Ok(())
        }
    }

    fn jo(a: u8, b: u8) -> u8 {
        (a << 4) | b
    }
    fn getr(s: String) -> Result<(u8, String)> {
        let s = s.trim_start();
        if s.len() < 2 {
            bail!("Expected register like RX");
        }
        let (reg, s) = s
            .find(|c: char| !c.is_alphanumeric())
            .map(|i| s.split_at(i))
            .unwrap_or_else(|| (s, ""));
        if let Some(num) = reg.to_ascii_lowercase().strip_prefix('r') {
            if let Ok(reg) = u8::from_str_radix(num, 16) {
                return Ok((reg, s.to_string()));
            }
        }
        bail!("Expected register, got '{reg}'");
    }
    fn getv(s: String) -> Result<(Val, String)> {
        let s = s.trim_start();
        let (val, s) = s
            .find(|c: char| c.is_whitespace() || c == ',')
            .map(|i| s.split_at(i))
            .unwrap_or_else(|| (s, ""));
        let s = s.to_string();
        let val = val.to_ascii_lowercase();
        let num = if let Some(hex) = val.strip_prefix("0x") {
            i32::from_str_radix(hex, 16)?
        } else if val.starts_with(|c: char| c.is_alphabetic() || c == '.') {
            return Ok((Ref(val), s));
        } else {
            val.parse()?
        };
        if num.abs() > 256 {
            bail!("Invalid value {val}");
        }
        let num = (if num < 0 { 256 + num } else { num }) as u8;
        Ok((Const(num), s))
    }
    fn comma(s: String) -> Result<String> {
        let s = s.trim_start();
        s.strip_prefix(',')
            .map(str::to_string)
            .ok_or_else(|| anyhow!("Expected comma between arguments"))
    }

    fn parse_line(s: &str, labels: &mut HashMap<String, u8>, res: &mut Output) -> Result<()> {
        fn p_rv(s: String, res: &mut Output, op: u8) -> Result<String> {
            let (reg, s) = getr(s)?;
            let (addr, s) = getv(comma(s)?)?;
            res.push(Const(jo(op, reg)))?;
            res.push(addr)?;
            Ok(s)
        }
        fn p_rrr(s: String, res: &mut Output, op: u8) -> Result<String> {
            let (r1, s) = getr(s)?;
            let (r2, s) = getr(comma(s)?)?;
            let (r3, s) = getr(comma(s)?)?;
            res.push(Const(jo(op, r1)))?;
            res.push(Const(jo(r2, r3)))?;
            Ok(s)
        }
        fn p_rr(s: String, res: &mut Output, op: u8) -> Result<String> {
            let (r1, s) = getr(s)?;
            let (r2, s) = getr(comma(s)?)?;
            res.push(Const(jo(op, r1)))?;
            res.push(Const(r2))?;
            Ok(s)
        }

        let mut s = s.trim();
        if let Some(index) = s.find(';') {
            s = &s[..index];
        }
        if let Some(index) = s.find(':') {
            let mut label = &s[..index];
            if let Some(index) = label.find('@') {
                let num = &label[index + 1..].to_ascii_lowercase();
                label = &label[..index];
                let addr = if let Some(hex) = num.strip_prefix("0x") {
                    u8::from_str_radix(hex, 16)?
                } else {
                    num.parse()?
                };
                res.pos = addr as usize;
            }
            if label.is_empty()
                || !{
                    let ch = label.chars().next().unwrap();
                    if ch == '.' {
                        label.chars().skip(1).all(identifier)
                    } else {
                        label.chars().all(identifier)
                    }
                }
            {
                bail!("Not a valid label: {label}");
            }
            if labels.contains_key(label) {
                bail!("Label {label} already exists");
            }
            if res.pos == 256 {
                bail!("Label at invalid position");
            }
            labels.insert(label.to_string(), res.pos as u8);
            s = &s[index + 1..];
        }
        let s = s.trim_start();
        if s.is_empty() {
            return Ok(());
        }
        let (mnemonic, s) = s.split_at(s.find(WS).unwrap_or(s.len()));
        let s = s.to_string();
        let s = match mnemonic.to_ascii_lowercase().as_ref() {
            "none" => {
                res.push(Const(0x00))?;
                res.push(Const(0x00))?;
                s
            }
            "loadm" => p_rv(s, res, 1)?,
            "loadb" => p_rv(s, res, 2)?,
            "storem" => p_rv(s, res, 3)?,
            "move" => {
                let (r1, s) = getr(s)?;
                let (r2, s) = getr(comma(s)?)?;
                res.push(Const(0x40))?;
                res.push(Const(jo(r2, r1)))?;
                s
            }
            "addi" => p_rrr(s, res, 5)?,
            "addf" => p_rrr(s, res, 6)?,
            "or" => p_rrr(s, res, 7)?,
            "and" => p_rrr(s, res, 8)?,
            "xor" => p_rrr(s, res, 9)?,
            "rot" => p_rv(s, res, 10)?,
            "jump" => p_rv(s, res, 11)?,
            "halt" => {
                res.push(Const(0xC0))?;
                res.push(Const(0x00))?;
                s
            }
            "loadp" => p_rr(s, res, 13)?,
            "storep" => p_rr(s, res, 14)?,
            "jumpl" => p_rv(s, res, 15)?,
            "db" => {
                let (val, s) = getv(s)?;
                res.push(val)?;
                s
            }
            _ => {
                bail!("Unknown mnemonic: {mnemonic}");
            }
        };
        let s = s.trim_start();
        if !s.is_empty() {
            bail!("Unexpected extra content: {s}");
        }
        Ok(())
    }
    let mut labels = HashMap::new();
    let mut res = Output::new();
    for (i, line) in code.split('\n').enumerate() {
        parse_line(line.trim(), &mut labels, &mut res)
            .context(format!("Error on line {}", i + 1))?;
    }
    res.mem
        .into_iter()
        .map(|val| match val {
            Const(val) => Ok(val),
            Ref(label) => {
                let (offset, label): (i32, &str) =
                    if let Some(index) = label.find('+').or_else(|| label.find('-')) {
                        (label[index..].parse()?, &label[..index])
                    } else {
                        (0, &label)
                    };
                Ok(u8::try_from(
                    labels
                        .get(label)
                        .ok_or_else(|| anyhow!("Unknown label: {label}"))
                        .copied()? as i32
                        + offset,
                )?)
            }
        })
        .collect()
}
