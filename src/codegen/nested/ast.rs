use std::fmt::{self, Write};

macro_rules! proxy_display {
    ( $target: ty ) => {
        impl fmt::Display for $target {
            fn fmt(&self, output: &mut fmt::Formatter) -> fmt::Result {
                let mut stream = AstStream::new(output, &self.1);
                AstFormat::fmt_ast(self, &mut stream)
            }
        }
    };
}

trait AstFormat {
    fn fmt_ast(&self, output: &mut AstStream) -> fmt::Result;
    fn fmt_key(&self, output: &mut AstStream<'_, '_>) -> fmt::Result {
        write!(output, "[")?;
        self.fmt_ast(output)?;
        write!(output, "]")
    }
}

#[derive(Debug)]
pub(crate) enum AstTarget {
    Lua,
    Typescript { output_dir: String },
}

pub(crate) struct AstStream<'a, 'b> {
    number_of_spaces: usize,
    indents: usize,
    is_start_of_line: bool,
    writer: &'a mut (dyn Write),
    target: &'b AstTarget,
}

impl<'a, 'b> AstStream<'a, 'b> {
    pub fn new(writer: &'a mut (dyn fmt::Write + 'a), target: &'b AstTarget) -> Self {
        Self {
            number_of_spaces: 4,
            indents: 0,
            is_start_of_line: true,
            writer,
            target,
        }
    }

    fn indent(&mut self) {
        self.indents += 1
    }

    fn unindent(&mut self) {
        if self.indents > 0 {
            self.indents -= 1
        }
    }

    fn begin_line(&mut self) -> fmt::Result {
        self.is_start_of_line = true;
        self.writer.write_char('\n')
    }
}

impl Write for AstStream<'_, '_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let mut is_first_line = true;

        for line in value.split('\n') {
            if is_first_line {
                is_first_line = false;
            } else {
                self.begin_line()?;
            }

            if !line.is_empty() {
                if self.is_start_of_line {
                    self.is_start_of_line = false;
                    self.writer.write_str(&format!(
                        "{: >1$}",
                        "",
                        self.number_of_spaces * self.indents
                    ))?;
                }

                self.writer.write_str(line)?;
            }
        }

        Ok(())
    }
}

proxy_display!(ReturnStatement);

#[derive(Debug)]
pub(crate) struct ReturnStatement(pub Expression, pub AstTarget);

impl AstFormat for ReturnStatement {
    fn fmt_ast(&self, output: &mut AstStream) -> fmt::Result {
        match output.target {
            AstTarget::Lua => {
                write!(output, "return ")
            }
            AstTarget::Typescript { output_dir } => {
                write!(output, "declare const {output_dir}: ")
            }
        }?;
        let result = self.0.fmt_ast(output);
        if let AstTarget::Typescript { output_dir } = output.target {
            write!(output, "\nexport = {output_dir}")?
        }
        result
    }
}

#[derive(Debug)]
pub(crate) enum Expression {
    String(String),
    Table(Table),
}

impl Expression {
    pub fn table(expressions: Vec<(Expression, Expression)>) -> Self {
        Self::Table(Table { expressions })
    }
}

impl AstFormat for Expression {
    fn fmt_ast(&self, output: &mut AstStream) -> fmt::Result {
        match self {
            Self::Table(val) => val.fmt_ast(output),
            Self::String(val) => val.fmt_ast(output),
        }
    }

    fn fmt_key(&self, output: &mut AstStream<'_, '_>) -> fmt::Result {
        match self {
            Self::Table(val) => val.fmt_key(output),
            Self::String(val) => val.fmt_key(output),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Table {
    pub expressions: Vec<(Expression, Expression)>,
}

impl AstFormat for Table {
    fn fmt_ast(&self, output: &mut AstStream<'_, '_>) -> fmt::Result {
        let assignment = match output.target {
            AstTarget::Lua => " = ",
            AstTarget::Typescript { .. } => ": ",
        };

        writeln!(output, "{{")?;
        output.indent();

        for (key, value) in &self.expressions {
            key.fmt_key(output)?;
            write!(output, "{assignment}")?;
            value.fmt_ast(output)?;
            writeln!(output, ",")?;
        }

        output.unindent();
        write!(output, "}}")
    }
}

impl AstFormat for String {
    fn fmt_ast(&self, output: &mut AstStream) -> fmt::Result {
        write!(output, "\"{}\"", self)
    }

    fn fmt_key(&self, output: &mut AstStream<'_, '_>) -> fmt::Result {
        if is_valid_identifier(self) {
            write!(output, "{}", self)
        } else {
            match output.target {
                AstTarget::Lua => write!(output, "[\"{}\"]", self),
                AstTarget::Typescript { .. } => write!(output, "\"{}\"", self),
            }
        }
    }
}

impl From<String> for Expression {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&'_ String> for Expression {
    fn from(value: &String) -> Self {
        Self::String(value.clone())
    }
}

impl From<&'_ str> for Expression {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<Table> for Expression {
    fn from(value: Table) -> Self {
        Self::Table(value)
    }
}

fn is_valid_ident_char_start(value: char) -> bool {
    value.is_ascii_alphabetic() || value == '_'
}

fn is_valid_ident_char(value: char) -> bool {
    value.is_ascii_alphanumeric() || value == '_'
}

fn is_valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();

    match chars.next() {
        Some(first) => {
            if !is_valid_ident_char_start(first) {
                return false;
            }
        }
        None => return false,
    }

    chars.all(is_valid_ident_char)
}
