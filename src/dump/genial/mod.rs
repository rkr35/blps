use std::fmt::{self, Display, Formatter};
use std::io::{self, Write};

macro_rules! ind {
    ($writer:expr, $fmt:expr, $($arg:tt),*) => (
        write!($writer.writer, concat!("{:indent$}", $fmt), "", $($arg,)* indent=$writer.indent)
    );

    ($writer:expr, $fmt:expr) => (ind!($writer, $fmt,));
}

macro_rules! ind_ln {
    ($writer:expr, $fmt:expr, $($arg:tt)*) => (ind!($writer, concat!($fmt, "\n"), $($arg)*));
    ($writer:expr, $fmt:expr) => (ind_ln!($writer, $fmt,));
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

    pub fn indent(&mut self) {
        self.indent += Self::INDENT;
    }

    pub fn undent(&mut self) {
        self.indent -= Self::INDENT;
    }
}

impl<W: Write> From<W> for Writer<W> {
    fn from(writer: W) -> Self {
        Self { writer, indent: 0 }
    }
}

pub trait WriterWrapper<W: Write> {
    fn writer(&mut self) -> &mut Writer<W>;

    fn line(&mut self, line: impl Display) -> Result<&mut Self, io::Error> {
        let writer = self.writer();
        ind_ln!(writer, "{}", line)?;
        Ok(self)
    }

    fn put(&mut self, put: impl Display) -> Result<&mut Self, io::Error> {
        let writer = self.writer();
        ind!(writer, "{}", put)?;
        Ok(self)
    }

    fn raw(&mut self, raw: impl Display) -> Result<&mut Self, io::Error> {
        let writer = self.writer();
        write!(writer.writer, "{}", raw)?;
        Ok(self)
    }
}

pub trait GenFunction<W: Write>: WriterWrapper<W> {
    fn function(
        &mut self,
        qualifiers: impl Display,
        name: impl Display,
    ) -> Result<Function<&mut W>, io::Error> {
        self.function_args(qualifiers, name, None::<(Nil, Nil)>)
    }

    fn write_function_header(
        &mut self,
        qualifiers: impl Display,
        name: impl Display,
    ) -> Result<(), io::Error> {
        let writer = self.writer();
        ind!(writer, "{}fn {}(", qualifiers, name)?;
        Ok(())
    }

    fn write_function_args<N: Display, T: Display>(
        &mut self,
        args: impl IntoIterator<Item = impl Into<Arg<N, T>>>,
    ) -> Result<(), io::Error> {
        let writer = self.writer();

        for arg in args {
            match arg.into() {
                Arg::Receiver(receiver) => write!(writer.writer, "{}, ", receiver)?,
                Arg::NameType(name, typ) => write!(writer.writer, "{}: {}, ", name, typ)?,
            }
        }

        Ok(())
    }

    fn function_args<N: Display, T: Display>(
        &mut self,
        qualifiers: impl Display,
        name: impl Display,
        args: impl IntoIterator<Item = impl Into<Arg<N, T>>>,
    ) -> Result<Function<&mut W>, io::Error> {
        self.write_function_header(qualifiers, name)?;
        self.write_function_args(args)?;

        let writer = self.writer();
        writeln!(writer.writer, ") {{")?;

        Ok(Function {
            writer: writer.nest(),
        })
    }

    fn function_args_ret<N: Display, T: Display>(
        &mut self,
        qualifiers: impl Display,
        name: impl Display,
        args: impl IntoIterator<Item = impl Into<Arg<N, T>>>,
        ret: impl Display,
    ) -> Result<Function<&mut W>, io::Error> {
        self.write_function_header(qualifiers, name)?;
        self.write_function_args(args)?;

        let writer = self.writer();
        writeln!(writer.writer, ") -> {} {{", ret)?;

        Ok(Function {
            writer: writer.nest(),
        })
    }
}

pub trait Gen<W: Write>: WriterWrapper<W> {
    fn structure(
        &mut self,
        vis: Visibility,
        name: impl Display,
    ) -> Result<Structure<&mut W>, io::Error> {
        let writer = self.writer();
        ind_ln!(writer, "{}struct {} {{", vis, name)?;

        Ok(Structure {
            writer: self.writer().nest(),
        })
    }

    fn enumeration(
        &mut self,
        vis: Visibility,
        name: impl Display,
    ) -> Result<Enumeration<&mut W>, io::Error> {
        let writer = self.writer();
        ind_ln!(writer, "{}enum {} {{", vis, name)?;

        Ok(Enumeration {
            writer: self.writer().nest(),
        })
    }

    fn imp(&mut self, target: impl Display) -> Result<Impl<&mut W>, io::Error> {
        let writer = self.writer();
        ind_ln!(writer, "impl {} {{", target)?;

        Ok(Impl {
            writer: self.writer().nest(),
        })
    }

    fn imp_trait(
        &mut self,
        r#trait: impl Display,
        target: impl Display,
    ) -> Result<Impl<&mut W>, io::Error> {
        let writer = self.writer();
        ind_ln!(writer, "impl {} for {} {{", r#trait, target)?;

        Ok(Impl {
            writer: self.writer().nest(),
        })
    }
}

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

macro_rules! impl_gen {
    ($($structure:ident)+) => {
        $(
            impl<W: Write> Gen<W> for $structure<W> {}
            impl<W: Write> GenFunction<W> for $structure<W> {}
        )+
    }
}

macro_rules! impl_closing_brace_drop {
    ($($structure:ident)+) => {
        $(
            impl<W: Write> Drop for $structure<W> {
                fn drop(&mut self) {
                    self.writer.undent();
                    ind_ln!(self.writer, "}}\n").unwrap();
                }
            }
        )+
    }
}

impl_writer_wrapper! { Scope Structure Enumeration Impl Function IfBlock }
impl_gen! { Scope Function IfBlock }
impl_closing_brace_drop! { Structure Enumeration Impl Function IfBlock }

impl<W: Write> GenFunction<W> for Impl<W> {}

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
            Self::Public => "pub ".fmt(f),
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
}

macro_rules! args {
    ($receiver:literal) => {{
        std::iter::once(Arg::<Nil, Nil>::Receiver($receiver))
    }};

    ($receiver:literal, $args:expr) => {{
        std::iter::once(Arg::Receiver($receiver)).chain($args.map(Arg::from))
    }};
}

pub enum Arg<N: Display, T: Display> {
    Receiver(&'static str),
    NameType(N, T),
}

impl<N: Display, T: Display> From<(N, T)> for Arg<N, T> {
    fn from((name, typ): (N, T)) -> Self {
        Self::NameType(name, typ)
    }
}

impl<D: Display> From<[D; 2]> for Arg<D, D> {
    fn from([name, typ]: [D; 2]) -> Self {
        Self::NameType(name, typ)
    }
}

impl<N: Display, T: Display, U: Into<Arg<N, T>> + Copy> From<&U> for Arg<N, T> {
    fn from(u: &U) -> Self {
        (*u).into()
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

pub struct Enumeration<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> Enumeration<W> {
    pub fn variant(&mut self, variant: impl Display) -> Result<&mut Self, io::Error> {
        ind_ln!(self.writer, "{},", variant)?;
        Ok(self)
    }
}

pub struct Impl<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> Impl<W> {}

pub struct Function<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> Function<W> {
    pub fn if_block(&mut self, r#if: impl Display) -> Result<IfBlock<&mut W>, io::Error> {
        ind_ln!(self.writer, "{} {{", r#if)?;

        Ok(IfBlock {
            writer: self.writer.nest(),
        })
    }
}

pub struct IfBlock<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> IfBlock<W> {
    pub fn else_block(&mut self, r#else: impl Display) -> Result<&mut Self, io::Error> {
        self.writer.undent();
        ind_ln!(self.writer, "}} {} {{", r#else)?;
        self.writer.indent();

        Ok(self)
    }
}

pub struct Nil;

impl Display for Nil {
    fn fmt(&self, _: &mut Formatter) -> Result<(), fmt::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
