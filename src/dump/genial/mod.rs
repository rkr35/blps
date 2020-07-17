use std::fmt::{self, Display, Formatter};
use std::io::{self, Write};

use typed_builder::TypedBuilder;

macro_rules! ind_ln {
    ($indent:expr, $($arg:tt)*) => (
        writeln!($indent.writer, "{:indent$}{rest}", "", indent=$indent.indent, rest=format_args!($($arg)*))
    )   
}

pub struct Indent<W: Write> {
    writer: W,
    indent: usize,
}

impl<W: Write> Indent<W> {
    const INDENT: usize = 4;

    pub fn new(writer: W) -> Indent<W> {
        Indent {
            writer,
            indent: 0,
        }
    }

    pub fn nest(&mut self) -> Indent<&mut W> {
        Indent {
            writer: &mut self.writer,
            indent: self.indent + Self::INDENT,
        }
    }

    pub fn unnest(&mut self) -> Indent<&mut W> {
        Indent {
            writer: &mut self.writer,
            indent: self.indent - Self::INDENT,
        }
    }
}

pub enum Visibility {
    Private,
    Public,
}

impl Default for Visibility {
    fn default() -> Self {
        Self::Private
    }
}

pub struct Scope<W: Write> {
    indent: Indent<W>,
}

impl<W: Write> Scope<W> {
    pub fn new(indent: Indent<W>) -> Scope<W> {
        Scope { indent }
    }

    pub fn structure(&mut self, name: impl Display) -> Result<Structure<&mut W>, io::Error> {
        ind_ln!(self.indent, "struct {} {{", name)?;
        Ok(Structure { indent: self.indent.nest() })
    }

    pub fn structure_attr(&mut self, attributes: impl Display, name: impl Display) -> Result<Structure<&mut W>, io::Error> {
        ind_ln!(self.indent, "{}", attributes)?;
        self.structure(name)
    }
}

pub struct Structure<W: Write> {
    indent: Indent<W>,
}

impl<W: Write> Drop for Structure<W> {
    fn drop(&mut self) {
        let indent = self.indent.unnest();
        ind_ln!(indent, "}}").unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str;

    #[test]
    fn scope_empty() {
        let mut buffer = vec![];

        {
            let _scope = Scope::new(Indent::new(&mut buffer));
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, "", "A newly created Scope should not emit anything.");
    }

    #[test]
    fn empty_structure() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Indent::new(&mut buffer));
            let _structure = scope.structure("Test");
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, "struct Test {\n}\n");
    }

    #[test]
    fn structure_repr_c() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Indent::new(&mut buffer));
            let _structure = scope.structure_attr("#[repr(C)]", "Test");
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("structure_repr_c.expected"));
    }
}