use std::fmt::{self, Display, Formatter};
use std::io::{self, Write};

macro_rules! ind_ln {
    ($writer:expr, $($arg:tt)*) => (
        writeln!($writer.writer, "{:indent$}{rest}", "", indent=$writer.indent, rest=format_args!($($arg)*))
    )   
}

pub struct Writer<W: Write> {
    writer: W,
    indent: usize,
}

impl<W: Write> Writer<W> {
    const INDENT: usize = 4;

    pub fn nest(&mut self) -> Writer<&mut W> {
        Writer {
            writer: &mut self.writer,
            indent: self.indent + Self::INDENT,
        }
    }

    pub fn unnest(&mut self) -> Writer<&mut W> {
        Writer {
            writer: &mut self.writer,
            indent: self.indent - Self::INDENT,
        }
    }
}

impl<W: Write> From<W> for Writer<W> {
    fn from(writer: W) -> Self {
        Self {
            writer,
            indent: 0
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

impl Display for Visibility {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match self {
            Self::Private => Ok(()),
            Self::Public => write!(f, "pub "),
        }
    }
}

pub struct Scope<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> Scope<W> {
    pub fn new(writer: Writer<W>) -> Scope<W> {
        Scope { writer }
    }

    pub fn structure(&mut self, attrs: Option<impl Display>, vis: Visibility, name: impl Display) -> Result<Structure<&mut W>, io::Error> {
        if let Some(attrs) = attrs {
            ind_ln!(self.writer, "{}\n{}struct {} {{", attrs, vis, name)?;
        } else {
            ind_ln!(self.writer, "{}struct {} {{", vis, name)?;
        }

        Ok(Structure {
            writer: self.writer.nest(),
        })
    }
}

pub struct Structure<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> Drop for Structure<W> {
    fn drop(&mut self) {
        let indent = self.writer.unnest();
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
            let _scope = Scope::new(Writer::from(&mut buffer));
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, "", "A newly created Scope should not emit anything.");
    }

    #[test]
    fn structure_empty() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let attrs: Option<&str> = None;
            let _structure = scope.structure(attrs, Visibility::default(), "Test");
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, "struct Test {\n}\n");
    }

    #[test]
    fn structure_empty_public() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let _structure = scope.structure(None::<&str>, Visibility::Public, "Test");
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, "pub struct Test {\n}\n");
    }

    #[test]
    fn structure_repr_c() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let _structure = scope.structure(
                Some("#[repr(C)]"),
                Visibility::Private,
                "Test"
            );
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("structure_repr_c.expected"));
    }

    #[test]
    fn structure_repr_c_public() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let _structure = scope.structure(
                Some("#[repr(C)]"),
                Visibility::Public,
                "Test"
            );
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("structure_repr_c_public.expected"));
    }
}