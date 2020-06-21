use std::borrow::Cow;

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

    pub fn emit(self, imp: &mut Impl, name: Cow<str>) {
        for (bit, field) in self.fields.into_iter().enumerate() {
            let mut normalized = field;

            let bytes = field.as_bytes();
            
            if field.len() >= 2 && bytes[0] == b'b' && bytes[1].is_ascii_uppercase() {
                normalized = &field[1..];
            }

            let normalized = normalized.to_snake_case();

            imp
                .new_fn(&format!("is_{}", normalized))
                .doc(field)
                .vis("pub")
                .arg_ref_self()
                .ret("bool")
                .line(format!("is_bit_set(self.{}, {})", name, bit));

            imp
                .new_fn(&format!("set_{}", normalized))
                .doc(field)
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

    pub fn emit(self, sdk: &mut Scope, structure: &'static str) {
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

            bitfield.emit(imp, name);
        }
    }
}