//! Model for template variables.
use crate::environment::Environment;
use anyhow::{anyhow, bail, Error};
use directories::BaseDirs;
use relative_path::{RelativePath, RelativePathBuf};
use serde::de;
use std::fmt;
use std::path::{Path, PathBuf};

/// A loaded template string.
#[derive(Debug, PartialEq, Eq)]
pub struct Template {
    parts: Vec<Part>,
}

impl fmt::Display for Template {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::Part::*;

        for part in &self.parts {
            match *part {
                Protocol(ref proto) => write!(fmt, "{}://", proto)?,
                Static(ref string) => string.fmt(fmt)?,
                Variable(ref var) => write!(fmt, "{{{}}}", var)?,
                Environ(ref env) => write!(fmt, "${}", env)?,
            }
        }

        Ok(())
    }
}

/// A single part in a template string.
#[derive(Debug, PartialEq, Eq)]
enum Part {
    /// Protocol part.
    Protocol(String),
    /// Static string.
    Static(String),
    /// A variable that should be looked up.
    Variable(String),
    /// An environment variable.
    Environ(String),
}

/// Trait to access variables.
pub trait Vars {
    /// Access a variable used for expansion.
    fn get(&self, k: &str) -> Option<&str>;
}

impl Template {
    /// Parse a template string, with variables delimited with `{var}`.
    pub fn parse(mut input: &str) -> Result<Template, Error> {
        let mut parts = Vec::new();

        if let Some(index) = input.find("://") {
            parts.push(Part::Protocol(input[..index].to_string()));
            input = &input[index + 3..];
        }

        let mut it = input.char_indices();

        let mut start = 0;

        while let Some((index, c)) = it.next() {
            match c {
                '{' => {
                    if index != start {
                        parts.push(Part::Static(input[start..index].to_string()));
                    }

                    let (end, var) = var(input, &mut it)?;
                    start = end;
                    parts.push(Part::Variable(var.to_string()));
                }
                '$' => {
                    if index != start {
                        parts.push(Part::Static(input[start..index].to_string()));
                    }

                    let (end, e) = environ(input, &mut it)?;
                    start = end;
                    parts.push(Part::Environ(e.to_string()));
                }
                _ => {}
            }
        }

        if !input[start..].is_empty() {
            parts.push(Part::Static(input[start..].to_string()));
        }

        return Ok(Template { parts });

        fn var(
            input: &str,
            mut it: impl Iterator<Item = (usize, char)>,
        ) -> Result<(usize, &str), Error> {
            let (start, _) = it.next().ok_or_else(|| anyhow!("missing char"))?;

            while let Some((index, c)) = it.next() {
                if c == '}' {
                    let (end, _) = it.next().ok_or_else(|| anyhow!("missing char"))?;
                    return Ok((end, &input[start..index]));
                }
            }

            bail!("missing closing '}'")
        }

        fn environ(
            input: &str,
            mut it: impl Iterator<Item = (usize, char)>,
        ) -> Result<(usize, &str), Error> {
            let (start, _) = it.next().ok_or_else(|| anyhow!("missing char"))?;

            for (index, c) in it {
                match c {
                    _ if c.is_uppercase() => continue,
                    '_' => continue,
                    _ => return Ok((index, &input[start..index])),
                }
            }

            Ok((input.len(), &input[start..]))
        }
    }

    /// Render as a relative path buffer.
    pub fn as_relative_path(
        &self,
        vars: impl Vars,
        environment: impl Environment,
    ) -> Result<Option<RelativePathBuf>, Error> {
        let protocol = |_: &str| {
            bail!("Relative paths do not support protocols");
        };

        let value = match self.render(vars, environment, protocol)? {
            Some(value) => value,
            None => return Ok(None),
        };

        Ok(Some(RelativePathBuf::from(value)))
    }

    /// Render as a path.
    pub fn as_path(
        &self,
        root: &Path,
        base_dirs: Option<&BaseDirs>,
        vars: impl Vars,
        environment: impl Environment,
    ) -> Result<Option<PathBuf>, Error> {
        let mut base = Some(root);

        let protocol = |proto: &str| {
            let b = match proto {
                "home" => base_dirs
                    .ok_or_else(|| anyhow!("Base dirs are required for home directory"))?
                    .home_dir(),
                proto => {
                    bail!("Unsupported protocol `{}`", proto);
                }
            };

            base = Some(b);
            Ok(())
        };

        let value = match self.render(vars, environment, protocol)? {
            Some(value) => value,
            None => return Ok(None),
        };

        let base = match base {
            Some(base) => base,
            None => {
                let mut buf = PathBuf::new();
                buf.extend(RelativePath::new(&value).components().map(|c| c.as_str()));
                return Ok(Some(buf));
            }
        };

        Ok(Some(RelativePath::new(&value).to_path(base)))
    }

    /// Simplified to render as string.
    pub fn as_string(
        &self,
        vars: impl Vars,
        environment: impl Environment,
    ) -> Result<Option<String>, Error> {
        self.render(vars, environment, |_| Ok(()))
    }

    /// Render the template variable.
    fn render(
        &self,
        vars: impl Vars,
        environment: impl Environment,
        mut protocol: impl FnMut(&str) -> Result<(), Error>,
    ) -> Result<Option<String>, Error> {
        use self::Part::*;
        use std::fmt::Write;

        let mut out = String::new();

        for part in &self.parts {
            match *part {
                Protocol(ref proto) => protocol(proto)?,
                Static(ref s) => out.write_str(s.as_str())?,
                Variable(ref var) => match vars.get(var) {
                    Some(value) => out.write_str(value)?,
                    None => return Ok(None),
                },
                Environ(ref environ) => match environment.var(environ)? {
                    Some(value) => out.write_str(value.as_str())?,
                    None => return Ok(None),
                },
            }
        }

        Ok(Some(out))
    }
}

impl<'de> de::Deserialize<'de> for Template {
    fn deserialize<D>(deserializer: D) -> Result<Template, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Template::parse(s.as_str()).map_err(|e| de::Error::custom(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use self::Part::*;
    use super::{Part, Template};
    use crate::facts::Facts;
    use std::collections::HashMap;

    #[test]
    fn test_parse_template() {
        let t = Template::parse("home://root/{foo}/$HOME/bar.yaml").unwrap();

        assert_eq!(
            t.parts,
            vec![
                Protocol("home".to_string()),
                Static("root/".to_string()),
                Variable("foo".to_string()),
                Static("/".to_string()),
                Environ("HOME".to_string()),
                Static("/bar.yaml".to_string()),
            ]
        );

        let facts = Facts::new(vec![("foo".to_string(), "baz".to_string())]);

        let mut environment = HashMap::new();
        environment.insert("HOME".to_string(), "home".to_string());

        assert_eq!(
            t.render(&facts, &environment, |_| Ok(()))
                .unwrap()
                .map(|n| n.to_string()),
            Some("root/baz/home/bar.yaml".to_string())
        );
    }
}
