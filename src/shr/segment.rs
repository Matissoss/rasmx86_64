// rasmx86_64 - src/shr/segment.rs
// -------------------------------
// made by matissoss
// licensed under MPL 2.0

use std::str::FromStr;

use crate::{
    conf::PREFIX_SEG,
    shr::{
        atype::{AType, ToAType},
        error::RASMError,
        mem::Mem,
        reg::{Purpose, Register},
        size::Size,
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub segment: Register,
    pub address: Mem,
}

impl FromStr for Segment {
    type Err = RASMError;
    fn from_str(str: &str) -> Result<Self, <Self as FromStr>::Err> {
        let mut chars_iter = str.chars();
        let mut tmp_buf = Vec::new();
        for c in chars_iter.by_ref() {
            if c == ':' {
                break;
            } else {
                tmp_buf.push(c);
            }
        }
        let str = String::from_iter(tmp_buf.iter());
        let seg_reg = match Register::from_str(&str) {
            Ok(r) => {
                if r.purpose() == Purpose::Sgmnt {
                    r
                } else {
                    return Err(RASMError::no_tip(
                        None,
                        Some("Tried to use register which purpose isn't for segments".to_string()),
                    ));
                }
            }
            Err(_) => {
                return Err(RASMError::no_tip(
                    None,
                    Some(format!("Couldn't create a segment register from {}", str)),
                ))
            }
        };
        tmp_buf = Vec::new();
        for c in chars_iter {
            tmp_buf.push(c);
        }
        let str = String::from_iter(tmp_buf.iter());
        let mem = Mem::new(&str, Size::Any)?;

        Ok(Self {
            segment: seg_reg,
            address: mem,
        })
    }
}

#[allow(clippy::to_string_trait_impl)]
impl ToString for Segment {
    fn to_string(&self) -> String {
        format!(
            "{}{}:{}",
            PREFIX_SEG,
            self.segment.to_string(),
            self.address.to_string()
        )
    }
}

impl ToAType for Segment {
    fn atype(&self) -> AType {
        AType::Memory(self.address.size().unwrap_or(Size::Unknown))
    }
}
