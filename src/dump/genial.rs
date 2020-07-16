use std::io::Write;

pub struct Scope<W> {
    writer: W,
}

impl<W: Write> Scope<W> {
    pub fn new(writer: W) -> Scope<W> {
        Scope { writer }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod scope {
        use super::*;

        #[test]
        fn empty() {
            let mut buffer = vec![];

            {
                let _scope = Scope::new(&mut buffer);
            }

            assert!(buffer.is_empty(), "A newly created Scope should be empty. Got: {:?}", buffer);
        }
    }
}