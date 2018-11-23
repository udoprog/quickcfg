//! Model for template variables.
use crate::{environment::Environment, facts::Facts};
use directories::BaseDirs;
use failure::{bail, format_err, Error};
use relative_path::{RelativePath, RelativePathBuf};
use serde::de;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    /// Path is interpreted as a verbatim path.
    Path,
    /// The configuration root.
    Root,
    /// The current users home directory.
    Home,
}

/// A loaded template string.
#[derive(Debug, PartialEq, Eq)]
pub struct Template {
    base: Base,
    parts: Vec<TemplatePart>,
}

/// A single part in a template string.
#[derive(Debug, PartialEq, Eq)]
pub enum TemplatePart {
    /// Static string.
    Static(String),
    /// A variable that should be looked up.
    Variable(String),
    /// An environment variable.
    Environ(String),
}

impl Template {
    /// Parse a template string, with variables delimited with `{var}`.
    pub fn parse(mut input: &str) -> Result<Template, Error> {
        let mut base = Base::Root;

        if let Some(index) = input.find(":") {
            match &input[..index] {
                "home" => base = Base::Home,
                "path" => base = Base::Path,
                base => bail!("unsupported base `{}`", base),
            }

            input = &input[index + 1..];
        }

        let mut parts = Vec::new();
        let mut it = input.char_indices();

        let mut start = 0;

        while let Some((index, c)) = it.next() {
            match c {
                '{' => {
                    if index != start {
                        parts.push(TemplatePart::Static(input[start..index].to_string()));
                    }

                    let (end, var) = var(input, &mut it)?;
                    start = end;
                    parts.push(TemplatePart::Variable(var.to_string()));
                }
                '$' => {
                    if index != start {
                        parts.push(TemplatePart::Static(input[start..index].to_string()));
                    }

                    let (end, e) = environ(input, &mut it)?;
                    start = end;
                    parts.push(TemplatePart::Environ(e.to_string()));
                }
                _ => {}
            }
        }

        if !input[start..].is_empty() {
            parts.push(TemplatePart::Static(input[start..].to_string()));
        }

        return Ok(Template { base, parts });

        fn var<'s>(
            input: &'s str,
            mut it: impl Iterator<Item = (usize, char)>,
        ) -> Result<(usize, &'s str), Error> {
            let (start, _) = it.next().ok_or_else(|| format_err!("missing char"))?;

            while let Some((index, c)) = it.next() {
                if c == '}' {
                    let (end, _) = it.next().ok_or_else(|| format_err!("missing char"))?;
                    return Ok((end, &input[start..index]));
                }
            }

            bail!("missing closing '}'")
        }

        fn environ<'s>(
            input: &'s str,
            mut it: impl Iterator<Item = (usize, char)>,
        ) -> Result<(usize, &'s str), Error> {
            let (start, _) = it.next().ok_or_else(|| format_err!("missing char"))?;

            while let Some((index, c)) = it.next() {
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
    pub fn render_as_relative_path(
        &self,
        facts: &Facts,
        environment: impl Environment,
    ) -> Result<Option<RelativePathBuf>, Error> {
        let value = match self.render(facts, environment)? {
            Some(value) => value,
            None => return Ok(None),
        };

        Ok(Some(RelativePathBuf::from(value)))
    }

    /// Render as a path.
    pub fn render_as_path(
        &self,
        root: &Path,
        base_dirs: Option<&BaseDirs>,
        facts: &Facts,
        environment: impl Environment,
    ) -> Result<Option<PathBuf>, Error> {
        let value = match self.render(facts, environment)? {
            Some(value) => value,
            None => return Ok(None),
        };

        let base = match self.base {
            Base::Home => base_dirs
                .ok_or_else(|| format_err!("base dirs are required for home directory"))?
                .home_dir(),
            Base::Root => root,
            Base::Path => {
                let mut buf = PathBuf::new();
                buf.extend(RelativePath::new(&value).components().map(|c| c.as_str()));
                return Ok(Some(buf));
            }
        };

        Ok(Some(RelativePath::new(&value).to_path(base)))
    }

    /// Render the template variable.
    pub fn render(
        &self,
        facts: &Facts,
        environment: impl Environment,
    ) -> Result<Option<String>, Error> {
        use self::TemplatePart::*;
        use std::fmt::Write;

        let mut out = String::new();

        for part in &self.parts {
            match *part {
                Static(ref s) => out.write_str(s.as_str())?,
                Variable(ref var) => match facts.get(var) {
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
    use self::TemplatePart::*;
    use super::{Base, Template, TemplatePart};
    use crate::{environment, facts::Facts};
    use std::collections::HashMap;

    #[test]
    fn test_parse_template() {
        let t = Template::parse("home:root/{foo}/$HOME/bar.yaml").unwrap();

        assert_eq!(
            t.parts,
            vec![
                Static("root/".to_string()),
                Variable("foo".to_string()),
                Static("/".to_string()),
                Environ("HOME".to_string()),
                Static("/bar.yaml".to_string()),
            ]
        );

        assert_eq!(t.base, Base::Home);

        let facts = Facts::new(vec![("foo".to_string(), "baz".to_string())]);

        let mut environment = HashMap::new();
        environment.insert("HOME".to_string(), "home".to_string());

        assert_eq!(
            t.render(&facts, environment::Custom(&environment))
                .unwrap()
                .map(|n| n.to_string()),
            Some("root/baz/home/bar.yaml".to_string())
        );
    }
}
