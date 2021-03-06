/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * espanso is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with espanso.  If not, see <https://www.gnu.org/licenses/>.
 */

use serde::{Serialize, Deserialize, Deserializer};
use crate::event::{KeyEvent, KeyModifier};
use crate::event::KeyEventReceiver;
use serde_yaml::Mapping;
use regex::Regex;
use std::path::PathBuf;
use std::fs;

pub(crate) mod scrolling;

#[derive(Debug, Serialize, Clone)]
pub struct Match {
    pub triggers: Vec<String>,
    pub content: MatchContentType,
    pub word: bool,
    pub passive_only: bool,
    pub propagate_case: bool,
    pub force_clipboard: bool,

    // Automatically calculated from the triggers, used by the matcher to check for correspondences.
    #[serde(skip_serializing)]
    pub _trigger_sequences: Vec<Vec<TriggerEntry>>,
}

#[derive(Debug, Serialize, Clone)]
pub enum MatchContentType {
    Text(TextContent),
    Image(ImageContent),
}

#[derive(Debug, Serialize, Clone)]
pub struct TextContent {
    pub replace: String,
    pub vars: Vec<MatchVariable>,

    #[serde(skip_serializing)]
    pub _has_vars: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct ImageContent {
    pub path: PathBuf,
}

impl <'de> serde::Deserialize<'de> for Match {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where
        D: Deserializer<'de> {

        let auto_match = AutoMatch::deserialize(deserializer)?;
        Ok(Match::from(&auto_match))
    }
}

impl<'a> From<&'a AutoMatch> for Match{
    fn from(other: &'a AutoMatch) -> Self {
        lazy_static! {
            static ref VAR_REGEX: Regex = Regex::new("\\{\\{\\s*(\\w+)\\s*\\}\\}").unwrap();
        };

        let mut triggers = if !other.triggers.is_empty() {
            other.triggers.clone()
        }else if !other.trigger.is_empty() {
            vec!(other.trigger.clone())
        }else{
            panic!("Match does not have any trigger defined: {:?}", other)
        };

        // If propagate_case is true, we need to generate all the possible triggers
        // For example, specifying "hello" as a trigger, we need to have:
        // "hello", "Hello", "HELLO"
        if other.propagate_case {
            // List with first letter capitalized
            let first_capitalized : Vec<String> = triggers.iter().map(|trigger| {
                let capitalized = trigger.clone();
                let mut v: Vec<char> = capitalized.chars().collect();
                v[0] = v[0].to_uppercase().nth(0).unwrap();
                v.into_iter().collect()
            }).collect();

            let all_capitalized : Vec<String> = triggers.iter().map(|trigger| {
                trigger.to_uppercase()
            }).collect();

            triggers.extend(first_capitalized);
            triggers.extend(all_capitalized);
        }

        let trigger_sequences = triggers.iter().map(|trigger| {
            // Calculate the trigger sequence
            let mut trigger_sequence = Vec::new();
            let trigger_chars : Vec<char> = trigger.chars().collect();
            trigger_sequence.extend(trigger_chars.into_iter().map(|c| {
                TriggerEntry::Char(c)
            }));
            if other.word {  // If it's a word match, end with a word separator
                trigger_sequence.push(TriggerEntry::WordSeparator);
            }

            trigger_sequence
        }).collect();


        let content = if let Some(replace) = &other.replace {  // Text match
            let new_replace = replace.clone();

            // Check if the match contains variables
            let has_vars = VAR_REGEX.is_match(replace);

            let content = TextContent {
                replace: new_replace,
                vars: other.vars.clone(),
                _has_vars: has_vars,
            };

            MatchContentType::Text(content)
        }else if let Some(image_path) = &other.image_path {  // Image match
            // On Windows, we have to replace the forward / with the backslash \ in the path
            let new_path = if cfg!(target_os = "windows") {
                image_path.replace("/", "\\")
            }else{
                image_path.to_owned()
            };

            // Calculate variables in path
            let new_path = if new_path.contains("$CONFIG") {
                let config_dir = crate::context::get_config_dir();
                let config_path = fs::canonicalize(&config_dir);
                let config_path = if let Ok(config_path) = config_path {
                    config_path.to_string_lossy().into_owned()
                }else{
                    "".to_owned()
                };
                new_path.replace("$CONFIG", &config_path)
            }else{
                new_path.to_owned()
            };

            let content = ImageContent {
                path: PathBuf::from(new_path)
            };

            MatchContentType::Image(content)
        }else {
            eprintln!("ERROR: no action specified for match {}, please specify either 'replace' or 'image_path'", other.trigger);
            std::process::exit(2);
        };

        Self {
            triggers,
            content,
            word: other.word,
            passive_only: other.passive_only,
            _trigger_sequences: trigger_sequences,
            propagate_case: other.propagate_case,
            force_clipboard: other.force_clipboard,
        }
    }
}

/// Used to deserialize the Match struct before applying some custom elaboration.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct AutoMatch {
    #[serde(default = "default_trigger")]
    pub trigger: String,

    #[serde(default = "default_triggers")]
    pub triggers: Vec<String>,

    #[serde(default = "default_replace")]
    pub replace: Option<String>,

    #[serde(default = "default_image_path")]
    pub image_path: Option<String>,

    #[serde(default = "default_vars")]
    pub vars: Vec<MatchVariable>,

    #[serde(default = "default_word")]
    pub word: bool,

    #[serde(default = "default_passive_only")]
    pub passive_only: bool,

    #[serde(default = "default_propagate_case")]
    pub propagate_case: bool,

    #[serde(default = "default_force_clipboard")]
    pub force_clipboard: bool,
}

fn default_trigger() -> String {"".to_owned()}
fn default_triggers() -> Vec<String> {Vec::new()}
fn default_vars() -> Vec<MatchVariable> {Vec::new()}
fn default_word() -> bool {false}
fn default_passive_only() -> bool {false}
fn default_replace() -> Option<String> {None}
fn default_image_path() -> Option<String> {None}
fn default_propagate_case() -> bool {false}
fn default_force_clipboard() -> bool {false}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MatchVariable {
    pub name: String,

    #[serde(rename = "type")]
    pub var_type: String,

    #[serde(default = "default_params")]
    pub params: Mapping,
}

fn default_params() -> Mapping {Mapping::new()}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TriggerEntry {
    Char(char),
    WordSeparator
}

pub trait MatchReceiver {
    fn on_match(&self, m: &Match, trailing_separator: Option<char>, trigger_offset: usize);
    fn on_enable_update(&self, status: bool);
    fn on_passive(&self);
}

pub trait Matcher : KeyEventReceiver {
    fn handle_char(&self, c: &str);
    fn handle_modifier(&self, m: KeyModifier);
    fn handle_other(&self);
}

impl <M: Matcher> KeyEventReceiver for M {
    fn on_key_event(&self, e: KeyEvent) {
        match e {
            KeyEvent::Char(c) => {
                self.handle_char(&c);
            },
            KeyEvent::Modifier(m) => {
                self.handle_modifier(m);
            },
            KeyEvent::Other => {
                self.handle_other();
            },
        }
    }
}


// TESTS

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_has_vars_should_be_false() {
        let match_str = r###"
        trigger: ":test"
        replace: "There are no variables"
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        match _match.content {
            MatchContentType::Text(content) => {
                assert_eq!(content._has_vars, false);
            },
            _ => {
                assert!(false);
            },
        }
    }

    #[test]
    fn test_match_has_vars_should_be_true() {
        let match_str = r###"
        trigger: ":test"
        replace: "There are {{one}} and {{two}} variables"
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        match _match.content {
            MatchContentType::Text(content) => {
                assert_eq!(content._has_vars, true);
            },
            _ => {
                assert!(false);
            },
        }
    }

    #[test]
    fn test_match_has_vars_with_spaces_should_be_true() {
        let match_str = r###"
        trigger: ":test"
        replace: "There is {{ one }} variable"
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        match _match.content {
            MatchContentType::Text(content) => {
                assert_eq!(content._has_vars, true);
            },
            _ => {
                assert!(false);
            },
        }
    }

    #[test]
    fn test_match_trigger_sequence_without_word() {
        let match_str = r###"
        trigger: "test"
        replace: "This is a test"
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        assert_eq!(_match._trigger_sequences[0][0], TriggerEntry::Char('t'));
        assert_eq!(_match._trigger_sequences[0][1], TriggerEntry::Char('e'));
        assert_eq!(_match._trigger_sequences[0][2], TriggerEntry::Char('s'));
        assert_eq!(_match._trigger_sequences[0][3], TriggerEntry::Char('t'));
    }

    #[test]
    fn test_match_trigger_sequence_with_word() {
        let match_str = r###"
        trigger: "test"
        replace: "This is a test"
        word: true
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        assert_eq!(_match._trigger_sequences[0][0], TriggerEntry::Char('t'));
        assert_eq!(_match._trigger_sequences[0][1], TriggerEntry::Char('e'));
        assert_eq!(_match._trigger_sequences[0][2], TriggerEntry::Char('s'));
        assert_eq!(_match._trigger_sequences[0][3], TriggerEntry::Char('t'));
        assert_eq!(_match._trigger_sequences[0][4], TriggerEntry::WordSeparator);
    }

    #[test]
    fn test_match_with_image_content() {
        let match_str = r###"
        trigger: "test"
        image_path: "/path/to/file"
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        match _match.content {
            MatchContentType::Image(content) => {
                assert_eq!(content.path, PathBuf::from("/path/to/file"));
            },
            _ => {
                assert!(false);
            },
        }
    }

    #[test]
    fn test_match_trigger_populates_triggers_vector() {
        let match_str = r###"
        trigger: ":test"
        replace: "This is a test"
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        assert_eq!(_match.triggers, vec![":test"])
    }

    #[test]
    fn test_match_triggers_are_correctly_parsed() {
        let match_str = r###"
        triggers:
          - ":test1"
          - :test2
        replace: "This is a test"
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        assert_eq!(_match.triggers, vec![":test1", ":test2"])
    }

    #[test]
    fn test_match_triggers_are_correctly_parsed_square_brackets() {
        let match_str = r###"
        triggers: [":test1", ":test2"]
        replace: "This is a test"
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        assert_eq!(_match.triggers, vec![":test1", ":test2"])
    }

    #[test]
    fn test_match_propagate_case() {
        let match_str = r###"
        trigger: "hello"
        replace: "This is a test"
        propagate_case: true
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        assert_eq!(_match.triggers, vec!["hello", "Hello", "HELLO"])
    }

    #[test]
    fn test_match_propagate_case_multi_trigger() {
        let match_str = r###"
        triggers: ["hello", "hi"]
        replace: "This is a test"
        propagate_case: true
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        assert_eq!(_match.triggers, vec!["hello", "hi", "Hello", "Hi", "HELLO", "HI"])
    }

    #[test]
    fn test_match_trigger_sequence_with_word_propagate_case() {
        let match_str = r###"
        trigger: "test"
        replace: "This is a test"
        word: true
        propagate_case: true
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();

        assert_eq!(_match._trigger_sequences[0][0], TriggerEntry::Char('t'));
        assert_eq!(_match._trigger_sequences[0][1], TriggerEntry::Char('e'));
        assert_eq!(_match._trigger_sequences[0][2], TriggerEntry::Char('s'));
        assert_eq!(_match._trigger_sequences[0][3], TriggerEntry::Char('t'));
        assert_eq!(_match._trigger_sequences[0][4], TriggerEntry::WordSeparator);

        assert_eq!(_match._trigger_sequences[1][0], TriggerEntry::Char('T'));
        assert_eq!(_match._trigger_sequences[1][1], TriggerEntry::Char('e'));
        assert_eq!(_match._trigger_sequences[1][2], TriggerEntry::Char('s'));
        assert_eq!(_match._trigger_sequences[1][3], TriggerEntry::Char('t'));
        assert_eq!(_match._trigger_sequences[1][4], TriggerEntry::WordSeparator);

        assert_eq!(_match._trigger_sequences[2][0], TriggerEntry::Char('T'));
        assert_eq!(_match._trigger_sequences[2][1], TriggerEntry::Char('E'));
        assert_eq!(_match._trigger_sequences[2][2], TriggerEntry::Char('S'));
        assert_eq!(_match._trigger_sequences[2][3], TriggerEntry::Char('T'));
        assert_eq!(_match._trigger_sequences[2][4], TriggerEntry::WordSeparator);
    }

    #[test]
    fn test_match_empty_replace_doesnt_crash() {
        let match_str = r###"
        trigger: "hello"
        replace: ""
        "###;

        let _match : Match = serde_yaml::from_str(match_str).unwrap();
    }
}