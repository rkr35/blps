use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Write};

use super::genial::{Gen, GenFunction, Impl, Scope, WriterWrapper};

use crate::args;

use heck::SnakeCase;

pub const FIELD: &str = "bitfield";

struct Bitfield {
    offset: u32,
    fields: Vec<&'static str>,
}

impl Bitfield {
    fn new(offset: u32, field: &'static str) -> Self {
        Self {
            offset,
            fields: vec![field],
        }
    }

    fn add(&mut self, field: &'static str) {
        self.fields.push(field);
    }

    pub fn emit(self, imp: &mut Impl<impl Write>, name: impl Display) -> Result<(), io::Error> {
        let mut counts: HashMap<Cow<str>, usize> = HashMap::new();

        let mut get_count = |s| *counts.entry(s).and_modify(|c| *c += 1).or_default();

        for (bit, field) in self.fields.into_iter().enumerate() {
            let field = {
                let mut f: Cow<str> = field.into();

                let count = get_count(field.into());

                if count > 0 {
                    f = format!("{}_{}", field, count).into();
                }

                f
            };

            let normalized = {
                let bytes = field.as_bytes();

                let has_hungarian_prefix =
                    field.len() >= 2 && bytes[0] == b'b' && bytes[1].is_ascii_uppercase();

                let f = if has_hungarian_prefix {
                    &field[1..]
                } else {
                    &field
                };

                let mut normalized = f.to_snake_case();

                let count = get_count(normalized.clone().into());

                if count > 0 {
                    normalized += "_";
                    normalized += &count.to_string();
                }

                normalized
            };

            imp
                .line(format_args!("// get {}", field))?
                .function_args_ret("pub ", format_args!("is_{}", normalized), args!("&self"), "bool")?
                .line(format_args!("is_bit_set(self.{}, {})", name, bit))?;

            imp
                .line(format_args!("// set {}", field))?
                .function_args("pub ", format_args!("set_{}", normalized), args!("&mut self", [("value", "bool")].iter()))?
                .line(format_args!("set_bit(&mut self.{}, {}, value);", name, bit))?;
        }

        Ok(())
    }
}

pub struct Bitfields {
    bitfields: Vec<Bitfield>,
}

#[derive(PartialEq, Eq)]
pub enum PostAddInstruction {
    Skip,
    EmitField,
}

impl Bitfields {
    pub fn new() -> Self {
        Self { bitfields: vec![] }
    }

    fn new_bitfield(&mut self, offset: u32, field: &'static str) -> PostAddInstruction {
        self.bitfields.push(Bitfield::new(offset, field));
        PostAddInstruction::EmitField
    }

    pub fn add(&mut self, offset: u32, field: &'static str) -> PostAddInstruction {
        if let Some(last) = self.bitfields.last_mut() {
            if last.offset == offset {
                last.add(field);
                PostAddInstruction::Skip
            } else {
                self.new_bitfield(offset, field)
            }
        } else {
            self.new_bitfield(offset, field)
        }
    }

    pub fn emit(self, sdk: &mut Scope<impl Write>, structure: &str) -> Result<(), io::Error> {
        if self.bitfields.is_empty() {
            return Ok(());
        }

        let mut imp = sdk.imp(structure)?;

        for (i, bitfield) in self.bitfields.into_iter().enumerate() {
            let name = Identifier {
                name: FIELD,
                id: i,
            };

            bitfield.emit(&mut imp, name)?;
        }

        Ok(())
    }
}

struct Identifier<N: Display, I: PartialOrd + Display + Default> {
    name: N,
    id: I,
}

impl<N: Display, I: PartialOrd + Display + Default> Display for Identifier<N, I> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        if self.id > I::default() {
            write!(f, "{}_{}", self.name, self.id)
        } else {
            write!(f, "{}", self.name)
        }
    }
}