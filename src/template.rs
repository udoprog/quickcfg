//! Model for template variables.
use crate::environment::Environment;
use failure::{bail, format_err, Error};
use relative_path::RelativePathBuf;
use serde::de;
use std::collections::HashMap;

/// A loaded template string.
#[derive(Debug, PartialEq, Eq)]
pub struct Template {
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
    pub fn parse(input: &str) -> Result<Template, Error> {
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

        return Ok(Template { parts });

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
        vars: &HashMap<String, String>,
        environment: impl Environment,
    ) -> Result<Option<RelativePathBuf>, Error> {
        let value = match self.render(vars, environment)? {
            Some(value) => value,
            None => return Ok(None),
        };

        Ok(Some(RelativePathBuf::from(value)))
    }

    /// Render the template variable.
    pub fn render(
        &self,
        variables: &HashMap<String, String>,
        environment: impl Environment,
    ) -> Result<Option<String>, Error> {
        use self::TemplatePart::*;
        use std::fmt::Write;

        let mut out = String::new();

        for part in &self.parts {
            match *part {
                Static(ref s) => out.write_str(s.as_str())?,
                Variable(ref var) => match variables.get(var) {
                    Some(value) => out.write_str(value.as_str())?,
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
    use super::{Template, TemplatePart};
    use crate::environment;
    use std::collections::HashMap;

    #[test]
    fn test_parse_template() {
        let t = Template::parse("root/{foo}/$HOME/bar.yaml").unwrap();

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

        let mut vars = HashMap::new();
        vars.insert("foo".to_string(), "baz".to_string());

        let mut environment = HashMap::new();
        environment.insert("HOME".to_string(), "home".to_string());

        assert_eq!(
            t.render(&vars, environment::Custom(&environment))
                .unwrap()
                .map(|n| n.to_string()),
            Some("root/baz/home/bar.yaml".to_string())
        );
    }
}
