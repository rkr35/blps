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

pub trait WriterWrapper<W: Write> {
    fn writer(&mut self) -> &mut Writer<W>;
}

pub trait Annotate<W: Write> : WriterWrapper<W> {
    fn annotate(&mut self, annotation: impl Display) -> Result<&mut Self, io::Error> {
        let writer = WriterWrapper::writer(self);
        ind_ln!(writer, "{}", annotation)?;
        Ok(self)
    }
}

impl<W: Write, V: WriterWrapper<W>> Annotate<W> for V {}

macro_rules! impl_writer_wrapper {
    ($($structure:ident)+) => {
        $(
            impl<W: Write> WriterWrapper<W> for $structure<W> {
                fn writer(&mut self) -> &mut Writer<W> {
                    &mut self.writer
                }
            }
        )+
    }
}

impl_writer_wrapper!{ Scope Structure Enumeration }

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

    pub fn structure(&mut self, vis: Visibility, name: impl Display) -> Result<Structure<&mut W>, io::Error> {
        ind_ln!(self.writer, "{}struct {} {{", vis, name)?;

        Ok(Structure {
            writer: self.writer.nest(),
        })
    }

    pub fn enumeration(&mut self, vis: Visibility, name: impl Display) -> Result<Enumeration<&mut W>, io::Error> {
        ind_ln!(self.writer, "{}enum {} {{", vis, name)?;

        Ok(Enumeration {
            writer: self.writer.nest(),
        })
    }
}

pub struct Structure<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> Structure<W> {
    pub fn field(&mut self, name: impl Display, typ: impl Display) -> Result<&mut Self, io::Error> {
        ind_ln!(self.writer, "{}: {},", name, typ)?;
        Ok(self)
    }
}

impl<W: Write> Drop for Structure<W> {
    fn drop(&mut self) {
        let indent = self.writer.unnest();
        ind_ln!(indent, "}}").unwrap();
    }
}

pub struct Enumeration<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> Enumeration<W> {
    pub fn variant(&mut self, variant: impl Display) -> Result<&mut Self, io::Error> {
        ind_ln!(self.writer, "{},", variant)?;
        Ok(self)
    }
}

impl<W: Write> Drop for Enumeration<W> {
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
            let _structure = scope.structure(Visibility::Private, "Test").unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, "struct Test {\n}\n");
    }

    #[test]
    fn structure_empty_public() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let _structure = scope.structure(Visibility::Public, "Test").unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, "pub struct Test {\n}\n");
    }

    #[test]
    fn structure_repr_c() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let _structure = scope
                .annotate("#[repr(C)]").unwrap()
                .structure(Visibility::Private,"Test").unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("structure_repr_c.expected"));
    }

    #[test]
    fn structure_repr_c_public() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let _structure = scope
                .annotate("#[repr(C)]").unwrap()
                .structure(Visibility::Public,"Test").unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("structure_repr_c_public.expected"));
    }

    #[test]
    fn structure_multiline_annotation() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let _structure = scope
                .annotate("#[repr(C)]").unwrap()
                .annotate("/// Second line").unwrap()
                .annotate("/// Third line").unwrap()
                .structure(Visibility::Private,"Test").unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("structure_multiline_annotation.expected"));
    }

    #[test]
    fn structure_single_field() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));

            let mut structure = scope.structure(
                Visibility::Private,
                "Test"
            ).unwrap();

            structure.field("field1", "u32").unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("structure_single_field.expected"));
    }

    #[test]
    fn structure_multiple_fields() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));

            let mut structure = scope.structure(
                Visibility::Private,
                "Test"
            ).unwrap();

            structure
                .field("field1", "u32")
                .unwrap()
                .field("field2", "Option<(bool, f32, String, i128)>")
                .unwrap()
                .field("field3", format_args!("[{}; {}]", "u8", 32))
                .unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("structure_multiple_fields.expected"));
    }

    #[test]
    fn structure_annotate_fields() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));

            let mut structure = scope.structure(
                Visibility::Private,
                "Test"
            ).unwrap();

            structure
                .annotate("// 0x0(0x4)").unwrap()
                .field("field1", "u32").unwrap()
                .annotate("#[test(attr)]").unwrap()
                .field("field2", "Option<(bool, f32, String, i128)>").unwrap()
                .annotate("// Multi-").unwrap()
                .annotate("// Line").unwrap()
                .field("field3", format_args!("[{}; {}]", "u8", 32)).unwrap()
                ;
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("structure_annotate_fields.expected"));
    }

    #[test]
    fn enum_single_variant() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let mut e = scope.enumeration(Visibility::Private, "TestEnum").unwrap();
            e.variant("TestVariant1").unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("enum_single_variant.expected"));
    }

    #[test]
    fn enum_single_variant_public() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let mut e = scope.enumeration(Visibility::Public, "TestEnum").unwrap();
            e.variant("TestVariant1").unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("enum_single_variant_public.expected"));
    }

    #[test]
    fn enum_single_variant_annotate() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let mut e = scope.enumeration(Visibility::Private, "TestEnum").unwrap();

            e
                .annotate("// Test annotation for first variant.").unwrap()
                .annotate("#[test(attr)]").unwrap()
                .variant("TestVariant1").unwrap();
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("enum_single_variant_annotate.expected"));
    }

    #[test]
    fn enum_multiple_variants() {
        let mut buffer = vec![];

        {
            let mut scope = Scope::new(Writer::from(&mut buffer));
            let mut e = scope.enumeration(Visibility::Private, "TestEnum").unwrap();

            e
                .variant("TestVariant1").unwrap()
                .variant("TestVariant2").unwrap()
                .variant("TestVariant3").unwrap()
                .variant("TestVariant4").unwrap()
                ;
        }

        let buffer = str::from_utf8(&buffer).unwrap();

        assert_eq!(buffer, include_str!("enum_multiple_variants.expected"));
    }
}