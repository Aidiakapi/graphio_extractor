use graphio_rs_data::{self as data, Int, Ratio};
use num_traits::identities::{One, Zero};
use crate::data::{Str, Metadata};

pub type Result<T> = ::std::result::Result<T, &'static str>;

type Iter = ::std::vec::IntoIter<String>;

pub fn read_line(p: &mut Iter) -> Result<String> {
    p.next().ok_or("unexpected end of data")
}

pub fn read_str(p: &mut Iter) -> Result<Str> {
    read_line(p).map(|x| Str::new(&x))
}

pub fn read_metadata(p: &mut Iter) -> Result<Metadata> {
    let localised_name = read_localised_str(p)?;
    let localised_description = read_optional_localised_str(p)?;
    Ok(Metadata {
        localised_name,
        localised_description,
        icon: None,
    })
}

pub fn read_localised_str(p: &mut Iter) -> Result<data::Str> {
    read_localised_str_internal(p, true).map(|x| x.unwrap())
}
pub fn read_optional_localised_str(p: &mut Iter) -> Result<Option<data::Str>> {
    read_localised_str_internal(p, false)
}

fn read_localised_str_internal(p: &mut Iter, required: bool) -> Result<Option<data::Str>> {
    let s = read_line(p)?;
    let mut iter = s.split('\x1f');
    let key = iter.next().ok_or("no key part in localised string")?;
    let value = iter.next().ok_or("no value part in localised string")?;
    if iter.next().is_some() {
        return Err("extra part in localised string");
    }

    Ok(
        if value.len() == 15 + key.len()
            && &value[0..14] == "Unknown key: \""
            && &value[value.len() - 1..] == "\""
        {
            if required {
                Some(Str::new(key))
            } else {
                None
            }
        } else {
            Some(Str::new(value))
        },
    )
}

pub fn read_usize(p: &mut Iter) -> Result<usize> {
    read_line(p)?.parse().map_err(|_| "cannot read usize")
}

pub fn read_int(p: &mut Iter) -> Result<Int> {
    read_line(p)?.parse().map_err(|_| "cannot read int")
}

// TODO: Improve approximating
pub fn read_ratio(p: &mut Iter) -> Result<Ratio> {
    let s = &read_line(p)?;
    if s.len() < 1 {
        return Err("expected ratio, got empty string");
    }
    let negative = s.starts_with('-');
    let s = if negative { &s[1..] } else { s };
    let period = s.find('.');
    let whole = if let Some(period) = period {
        if let Some(_) = s[period + 1..].find('e') {
            return Err("scientific notation not supported");
        }
        &s[0..period]
    } else {
        s
    };

    let mut base = Int::zero();
    for char in whole.chars() {
        let d = char
            .to_digit(10)
            .ok_or("unexpected non-digit in string to ratio")?;
        base *= 10;
        base += d;
    }

    let whole = Ratio::new_raw(base, Int::one());
    let fraction = if let Some(period) = period {
        let approx = s[period..]
            .parse::<f64>()
            .ok()
            .ok_or("cannot parse fractional part as f64 for ratio")?;

        if approx <= 0.0 {
            Ratio::zero()
        } else {
            let (mut closest_delta, mut closest_num, mut closest_den) = (approx, 0, 1);

            // PERF: Very inefficient
            'outer: for den in 1..1001 {
                for num in 1..den {
                    let delta = (approx - (num as f64) / (den as f64)).abs();
                    if delta < closest_delta {
                        closest_delta = delta;
                        closest_num = num as i64;
                        closest_den = den as i64;
                        if delta <= 0.00000001 {
                            break 'outer;
                        }
                    }
                }
            }

            Ratio::new(Int::from(closest_num), Int::from(closest_den))
        }
    } else {
        Ratio::zero()
    };

    Ok(if negative {
        -(whole + fraction)
    } else {
        whole + fraction
    })
}

pub struct AllowedEffects {
    pub energy: bool,
    pub speed: bool,
    pub productivity: bool,
    pub pollution: bool,
}

pub fn read_allowed_effects(p: &mut Iter) -> Result<AllowedEffects> {
    let line = read_line(p)?;
    if line.len() != 4 {
        return Err("expected allowed_effects to be 4 bits");
    }
    let bytes = line.as_bytes();
    #[inline(always)]
    fn parse_bit(c: u8) -> Result<bool> {
        match c {
            b'0' => Ok(false),
            b'1' => Ok(true),
            _ => Err("expected 0 or 1 as bit value"),
        }
    }

    let energy = parse_bit(bytes[0])?;
    let speed = parse_bit(bytes[1])?;
    let productivity = parse_bit(bytes[2])?;
    let pollution = parse_bit(bytes[3])?;

    Ok(AllowedEffects {
        energy,
        speed,
        productivity,
        pollution,
    })
}
