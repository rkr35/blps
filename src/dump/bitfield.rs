use std::borrow::Cow;
use std::collections::HashMap;

use codegen::{Impl, Scope};
use heck::SnakeCase;

pub const FIELD: &str = "bitfield";

struct Bitfield {
    offset: u32,
    fields: Vec<&'static str>,
}

impl Bitfield {
    fn new(offset: u32, field: &'static str) -> Self {
        Self { offset, fields: vec![field] }
    }

    fn add(&mut self, field: &'static str) {
        self.fields.push(field);
    }

    pub fn emit(self, imp: &mut Impl, name: &str) {
        let mut counts: HashMap<Cow<str>, usize> = HashMap::new();

        let mut get_count = |s| *counts
            .entry(s)
            .and_modify(|c| *c += 1)
            .or_default();

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
                
                let has_hungarian_prefix = field.len() >= 2
                    && bytes[0] == b'b'
                    && bytes[1].is_ascii_uppercase();

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
                .new_fn(&format!("is_{}", normalized))
                .doc(&format!("get {}", field))
                .vis("pub")
                .arg_ref_self()
                .ret("bool")
                .line(format!("is_bit_set(self.{}, {})", name, bit));

            imp
                .new_fn(&format!("set_{}", normalized))
                .doc(&format!("set {}", field))
                .vis("pub")
                .arg_mut_self()
                .arg("value", "bool")
                .line(format!("set_bit(&mut self.{}, {}, value);", name, bit));
        }
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

    pub fn emit(self, sdk: &mut Scope, structure: &str) {
        if self.bitfields.is_empty() {
            return;
        }

        let imp = sdk.new_impl(structure);

        for (i, bitfield) in self.bitfields.into_iter().enumerate() {
            let name: Cow<str> = if i > 0 {
                format!("{}_{}", FIELD, i).into()
            } else {
                FIELD.into()
            };

            bitfield.emit(imp, &name);
        }
    }
}