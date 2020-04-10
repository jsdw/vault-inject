use std::collections::{ HashSet };
use std::str::FromStr;
use once_cell::sync::Lazy;
use regex::Regex;
use anyhow::{ Result, anyhow };

#[derive(Debug,Clone)]
pub struct Template {
    pieces: Vec<Piece>,
    re: Regex
}

impl Template {

    /// Instantiate a new template given a string
    /// like 'foo {bar} wibble'
    pub fn new(s: &str) -> Result<Template> {
        Template::from_str(s)
    }

    /// Given a string, attempt to match this template. If we
    /// succeed, return those matches. If not, return None
    pub fn matches<'t>(&self, s: &'t str) -> Option<Matches<'t>> {
        self.re.captures(s).map(|caps| Matches(caps))
    }

    /// Convert this template into a string using the matches obtained
    /// from another template. If a param that's used isn't provided,
    /// it's replaced with an empty string
    pub fn stringify<M: Matcher>(&self, matches: &M) -> String {
        let mut out = String::new();
        for piece in &self.pieces {
            match piece {
                Piece::Str(s) => {
                    out.push_str(s);
                },
                Piece::Param(name) => {
                    let m = matches.get_match(name).unwrap_or("");
                    out.push_str(m);
                }
            }
        }
        out
    }

    /// Is it possible to stringify this template from the one
    /// provided without leaving gaps? In order for this to be true,
    /// the other template must contain all of the named {params}
    /// that this one does
    pub fn can_stringify_from(&self, other: &Template) -> bool {
        let mut other_has = HashSet::new();
        for piece in &other.pieces {
            if let Piece::Param(name) = piece {
                other_has.insert(name);
            }
        }

        for piece in &self.pieces {
            if let Piece::Param(name) = piece {
                if !other_has.contains(name) {
                    return false
                }
            }
        }

        true
    }

}

impl PartialEq for Template {
    fn eq(&self, other: &Self) -> bool {
        // if pieces are the same, the regex will be also:
        self.pieces == other.pieces
    }
}

impl FromStr for Template {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Template> {

        static RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"(.*?)(\{\s*([a-zA-Z][a-zA-Z0-9_-]*)\s*\})").unwrap()
        });

        // Seen params:
        let mut seen_params = HashSet::new();

        // Keep track of the pieces of our template:
        let mut out_pieces = Vec::new();

        // Next, find the patterns in our path and build up
        // our regex based on them:
        let mut last_idx = 0;
        for cap in RE.captures_iter(s) {

            let normal_str = cap.get(1).unwrap().as_str();
            let all_template_param = cap.get(2).unwrap();
            let template_param_name = cap.get(3).unwrap().as_str();

            if !seen_params.insert(template_param_name) {
                return Err(anyhow!("The paramater '{}' was used more than once", template_param_name));
            }

            if !normal_str.is_empty() {
                out_pieces.push(Piece::Str(normal_str.to_owned()));
            }

            out_pieces.push(Piece::Param(template_param_name.to_owned()));
            last_idx = all_template_param.end();
        }

        // Remember to push the rest of the string into the regex:
        out_pieces.push(Piece::Str(s[last_idx..].to_owned()));

        // Build up a regular expression from the pieces that we can match with:
        let mut out_regex = String::new();
        out_regex.push('^');
        for piece in &out_pieces {
            match piece {
                Piece::Str(s) => {
                    out_regex.push_str(&regex::escape(s));
                },
                Piece::Param(name) => {
                    out_regex.push_str(&format!("(?P<{}>.+?)", name));
                }
            }
        }
        out_regex.push('$');

        Ok(Template {
            pieces: out_pieces,
            re: Regex::from_str(&out_regex).unwrap()
        })
    }
}


#[derive(Clone,Debug,PartialEq)]
enum Piece {
    Str(String),
    Param(String)
}

pub struct Matches<'t>(regex::Captures<'t>);

pub trait Matcher {
    fn get_match<'a>(&'a self, key: &str) -> Option<&'a str>;
}
impl <'t> Matcher for Matches<'t> {
    fn get_match(&self, key: &str) -> Option<&str> {
        self.0.name(key).map(|m| m.as_str())
    }
}

#[cfg(test)]
mod test {

    use super::*;

    // Allow providing a vec of keyvalue pairs to substitute templates with:
    impl Matcher for Vec<(&str,&str)> {
        fn get_match(&self, key: &str) -> Option<&str> {
            self.iter().find(|(k,_v)| *k == key).map(|(_k,v)| *v)
        }
    }

    #[test]
    fn stringify_template() {

        let cases = vec![
            ("foo_{bar}", vec![("bar","hello")], "foo_hello"),
            ("foo_{bar}", vec![("bar","wibble")], "foo_wibble"),
            ("foo_{ bar }", vec![("bar","wibble")], "foo_wibble"),
            ("{a}", vec![("a","b")], "b"),
            ("a", vec![("a","b")], "a"),
            ("{a},{b},{c}", vec![("a","A"),("b","B"),("c","C")], "A,B,C"),
            ("{a},{b},{c}", vec![("a","A"),("b","B")], "A,B,"),
        ];

        for (tmpl_str, subs, expected) in cases {
            let tmpl = Template::new(tmpl_str).expect("Could not instantiate template");
            let actual = tmpl.stringify(&subs);
            assert_eq!(actual, expected, "Stringified template does not match expected");
        }

    }

    #[test]
    fn match_template() {

        let cases = vec![
            ("foo {bar} wibble", "foo 12345 wibble", true),
            ("foo {bar} wibble", "foo some words here wibble", true),
            // Nothing can exist before or after the tmpl string:
            ("foo {bar} wibble", "foo 12345 wibble no", false),
            ("foo {bar} wibble", "no foo 12345 wibble", false),
            // A capture matches whatever it can:
            ("{ anything }", "blah 123 -=12-090 :@~£", true),
            ("{ anything }", "a", true),
            // A capture can't match nothing at all:
            ("{ anything }", "", false),
            // Multiple captures are non-greedily handled:
            ("{a}_{b}_c_{d}", "A_A_A_b_b_b_c_dDdDdD", true),
            ("{a}_{b}_c_{d}", "A_A_A_b_b_b_z_dDdDdD", false),
        ];

        for (tmpl_str, match_str, does_match) in cases {
            let tmpl = Template::new(tmpl_str).expect("Could not instantiate template");
            let actual_does_match = tmpl.matches(match_str).is_some();
            if does_match {
                assert!(actual_does_match, "'{}' should match the string '{}'", tmpl_str, match_str);
            } else {
                assert!(!actual_does_match, "'{}' should NOT match the string '{}'", tmpl_str, match_str);
            }
        }

    }

}