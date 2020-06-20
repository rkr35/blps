use std::fmt::{self, Display, Formatter};

use codegen::Scope;

const INDENT: &str = "    ";

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
}

impl Display for Bitfield {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let fields: Vec<String> = self
            .fields
            .iter()
            .enumerate()
            .map(|(i, field)| format!("{indent}{indent}{indent}{} 1 << {}", field, i, indent=INDENT))
            .collect();

        let fields = fields.join("\n");

        write!(f, "{}", fields)
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
        
        sdk.raw(&{
            let bitfields: Vec<String> = self
                .bitfields
                .into_iter()
                .enumerate()
                .map(|(i, bitfield)| {
                    let name = if i > 0 {
                        format!("_{}", i)
                    } else {
                        String::new()
                    };

                    format!("{indent}{indent}bitfield{}:\n{}", name, bitfield, indent=INDENT)
                })
                .collect();
            
            let bitfields = bitfields.join("\n\n");

            format!("/*\n{indent}Bitfields for {}:\n\n{}\n*/", structure, bitfields, indent=INDENT)
        });

        /*
            Bitfields for `structure`:

                bitfield:
                    a 1 << 0
                    b 1 << 2
                    c 1 << 3
                    ...

                bitfield_1:
                    x 1 << 0
                    y 1 << 1
                    ...

                ...
        */
    }
}