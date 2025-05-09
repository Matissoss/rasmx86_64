// rasmx86_64 - src/shr/error.rs
// -----------------------------
// made by matissoss
// licensed under MPL 2.0

use crate::CLI;
use std::{
    fmt::{Display, Error, Formatter},
    fs::{File, OpenOptions},
    io::Read,
    path::PathBuf,
    sync::LazyLock,
};

static ERR_CTX: LazyLock<(File, PathBuf)> = LazyLock::new(|| {
    let path = PathBuf::from(CLI.get_arg("-i").unwrap());
    (
        OpenOptions::new()
            .write(false)
            .append(false)
            .truncate(false)
            .create_new(false)
            .create(false)
            .read(true)
            .open(&path)
            .unwrap(),
        path,
    )
});

static FILE: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut buf = String::new();
    (&ERR_CTX.0).read_to_string(&mut buf).unwrap();
    buf.lines().map(|s| s.to_string()).collect::<Vec<String>>()
});

use crate::color::{BaseColor, ColString, Modifier};

#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum ExceptionType {
    Warn,
    Error,
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RASMError {
    line: Option<usize>,
    etype: ExceptionType,
    msg: Option<String>,
    tip: Option<String>,
}

impl Display for RASMError {
    fn fmt(&self, frm: &mut Formatter<'_>) -> Result<(), Error> {
        let ctx = if let Some(line) = self.line { Some(&FILE[line]) } else { None };

        writeln!(
            frm,
            "{}:\n\tin {}{}{}{}{}",
            self.etype,
            ColString::new(ERR_CTX.1.to_string_lossy()).set_color(BaseColor::YELLOW),
            if let Some(line) = self.line {
                format!(" at line {}", ColString::new(line).set_color(BaseColor::YELLOW))
            } else {"".to_string()},
            if let Some(ctx) = ctx{
                format!("\n\t{}",
                ColString::new(ctx)
                .set_color(BaseColor::GREEN)
                .set_modf(Modifier::Bold))
            } else {"".to_string()},
            if let Some(msg) = &self.msg {
                format!("\n\t---\n\t{}", msg)
            } else {
                "".to_string()
            },
            if let Some(tip) = &self.tip {
                format!(
                    "\n\t{} {}",
                    ColString::new("tip:")
                        .set_color(BaseColor::BLUE)
                        .set_modf(Modifier::Bold),
                    ColString::new(tip)
                        .set_color(BaseColor::BLUE)
                        .set_modf(Modifier::Bold)
                )
            } else {
                "".to_string()
            }
        )
    }
}

impl Display for ExceptionType {
    fn fmt(&self, frm: &mut Formatter<'_>) -> Result<(), Error> {
        write!(
            frm,
            "{}",
            if let Self::Warn = self {
                ColString::new("warn").set_color(BaseColor::YELLOW)
            } else if let Self::Error = self {
                ColString::new("error").set_color(BaseColor::RED)
            } else {
                ColString::new("info").set_color(BaseColor::BLUE)
            }
        )
    }
}

impl RASMError {
    pub fn no_tip(line: Option<usize>, msg: Option<impl ToString>) -> Self{
        Self {
            line,
            etype: ExceptionType::Error,
            msg: if let Some(m) = msg {
                Some(m.to_string())
            } else { None },
            tip: None
        }
    }
    pub fn with_tip(line: Option<usize>, msg: Option<impl ToString>, tip: Option<impl ToString>) -> Self{
        Self {
            line,
            etype: ExceptionType::Error,
            msg: if let Some(m) = msg {
                Some(m.to_string())
            } else { None },
            tip: if let Some(t) = tip {
                Some(t.to_string())
            } else { None },
        }
    }
    pub fn get_line(&self) -> Option<&usize> {
        self.line.as_ref()
    }
    pub fn set_line(&mut self, newline: usize) {
        self.line = Some(newline);
    }
}

pub struct Blank;

#[allow(clippy::to_string_trait_impl)]
impl ToString for Blank{
    fn to_string(&self) -> String{
        String::new()
    }
}
