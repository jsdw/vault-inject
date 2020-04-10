use std::str::FromStr;
use anyhow::{ anyhow, Result };
use crate::template::Template;

/// A mapping from secret to environment variable
#[derive(Clone,Debug)]
pub struct SecretMapping {
    path: String,
    key: Template,
    processors: Vec<String>,
    env_var: Template,
}

impl SecretMapping {
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn processors(&self) -> &[String] {
        &self.processors
    }

    /// If the provided key matches this mapping, return the
    /// environment variable name it corresponds to, else None.
    pub fn env_var_from_key(&self, key: &str) -> Option<String> {
        let matches = self.key.matches(key)?;
        let env_var_name = self.env_var.stringify(&matches);
        Some(env_var_name)
    }
}

impl FromStr for SecretMapping {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<SecretMapping> {
        let idx = s.find('=')
            .ok_or_else(|| anyhow!("Expected secrets of the form 'ENV_VAR=path/to/secret/key' but got '{}'", s))?;

        let env_var_str = s[0..idx].trim();
        let secret_str = &s[idx+1..];

        let secret_str_bits = secret_str
            .split('|')
            .map(|s| s.trim())
            .collect::<Vec<_>>();

        for (idx,cmd) in secret_str_bits.iter().enumerate() {
            if cmd.is_empty() {
                let n = idx+1;
                return Err(anyhow!("Every '|' must forward to a command, but command {} of '{}' is missing", n, s))
            }
        }

        let (&path_and_key_str, processor_strs) = secret_str_bits
            .split_first()
            .ok_or_else(|| anyhow!("Expected secret values of the form 'path/to/secret/key [| command ...]' but got '{}'", secret_str))?;

        let (path_str, key_str) = split_secret_path_and_key(path_and_key_str)
            .ok_or_else(|| anyhow!("Expected the secret path to have at least one '/' in it but got '{}'", path_and_key_str))?;

        let path = path_str.trim_start_matches('/').to_owned();

        let key = Template::new(key_str)
            .map_err(|e| anyhow!("Invalid key template '{}': {}", key_str, e))?;
        let env_var = Template::new(env_var_str)
            .map_err(|e| anyhow!("Invalid environment variable template '{}': {}", env_var_str, e))?;
        if !env_var.can_stringify_from(&key) {
            return Err(anyhow!("The environment variable pattern '{}' contains template parameters not seen in the corresponding key '{}'", env_var_str, key_str));
        }

        let processors = processor_strs
            .iter()
            .map(|&s| s.to_owned())
            .collect();

        Ok(SecretMapping {
            path,
            key,
            env_var,
            processors
        })
    }
}

fn split_secret_path_and_key(s: &str) -> Option<(&str, &str)> {
    let idx = s.rfind('/')?;
    if idx == 0 { return None  }
    Some((&s[0..idx], &s[idx+1..]))
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_string_to_secretmapping() {

        let cases = vec![
            // ###############
            // ### Allowed ###
            // ###############

            // Basic paths (we strip leading but not trailing '/'):
            ("FOO = /hello/foo/bar", Some(("FOO", "hello/foo", "bar", vec![]))),
            // We ignore various whitespace:
            ("FOO= /hello/foo/bar ", Some(("FOO", "hello/foo", "bar", vec![]))),
            ("FOO=/hello/foo/bar ", Some(("FOO", "hello/foo", "bar", vec![]))),
            (" FOO=/hello/foo/bar ", Some(("FOO", "hello/foo", "bar", vec![]))),
            // We allow secrets to be piped through commands:
            ("FOO= /hello/foo/bar | base64", Some(("FOO", "hello/foo", "bar", vec!["base64"]))),
            ("FOO= /hello/foo/bar | base64 | rev", Some(("FOO", "hello/foo", "bar", vec!["base64", "rev"]))),
            ("FOO=/hello/foo/bar|base64|rev", Some(("FOO", "hello/foo", "bar", vec!["base64", "rev"]))),
            ("FOO=/hello/foo/bar|base64| rev ", Some(("FOO", "hello/foo", "bar", vec!["base64", "rev"]))),
            // You can use parameters:
            ("{bar} = /hello/foo/{bar} ", Some(("{bar}", "hello/foo", "{bar}", vec![]))),
            ("FOO_{bar} = /hello/foo/{bar} ", Some(("FOO_{bar}", "hello/foo", "{bar}", vec![]))),

            // ###################
            // ### NOT Allowed ###
            // ###################

            // You must have a path:
            ("FOO", None),
            // The path string must have at least one '/' in it (path/key):
            ("FOO = /hello", None),
            // You must use '='
            ("FOO /hello/lark", None),
            // You can't have empty commands:
            ("FOO = /hello/lark |", None),
            ("FOO = /hello/lark ||", None),
            ("FOO = /hello/lark ||rev", None),
        ];

        for (s, res) in cases {
            match res {
                Some((env, path, key, processors)) => {
                    let processor_strings: Vec<String> = processors
                        .into_iter()
                        .map(|s: &str| s.to_owned())
                        .collect();
                    let mapping = match SecretMapping::from_str(s) {
                        Ok(mapping) => mapping,
                        Err(err) => panic!("String '{}' is not a valid SecretMapping: {:?}", s, err)
                    };

                    assert_eq!(Template::new(env).unwrap(), mapping.env_var, "Environment variable doesn't match expected");
                    assert_eq!(Template::new(key).unwrap(), mapping.key, "Key doesn't match expected");
                    assert_eq!(path.to_owned(), mapping.path, "Path doesn't match expected");
                    assert_eq!(processor_strings, mapping.processors, "Piped commands don't match expected");
                },
                None => {
                    if let Ok(mapping) = SecretMapping::from_str(s) {
                        panic!("Did not expect '{}' to be valid, but it was: {:?}", s, mapping);
                    }
                }
            }
        }

    }

}