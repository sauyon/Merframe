use std::collections::HashMap;

use serde::Deserialize;

use crate::error::{DataError, Result};
use crate::markup::without_markup;

#[derive(Debug, Deserialize)]
struct RawNightwaveExport {
    challenges: HashMap<String, RawNightwaveAct>,
}

#[derive(Debug, Deserialize)]
struct RawNightwaveAct {
    name: String,
    description: String,
    required: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NightwaveAct {
    pub title: String,
    pub description_template: String,
    pub required: u32,
}

#[derive(Debug, Clone, Default)]
pub struct NightwaveActs {
    by_name: HashMap<String, NightwaveAct>,
}

impl NightwaveActs {
    pub fn from_json(export: &str, dictionary: &str) -> Result<Self> {
        let export: RawNightwaveExport = serde_json::from_str(export)
            .map_err(|source| DataError::Parse("Nightwave act", source))?;
        let dictionary: HashMap<String, String> = serde_json::from_str(dictionary)
            .map_err(|source| DataError::Parse("Nightwave dictionary", source))?;

        let by_name = export
            .challenges
            .into_iter()
            .filter_map(|(path, act)| {
                let name = path
                    .rsplit_once('/')
                    .map_or(path.as_str(), |(_, name)| name);
                let act = NightwaveAct {
                    title: dictionary.get(&act.name)?.clone(),
                    description_template: without_markup(dictionary.get(&act.description)?)
                        .trim_end()
                        .trim_end_matches('.')
                        .to_owned(),
                    required: act.required,
                };
                Some((name.to_lowercase(), act))
            })
            .collect();
        Ok(Self { by_name })
    }

    pub fn get(&self, challenge: &str) -> Option<&NightwaveAct> {
        self.by_name.get(&challenge.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPORT: &str = include_str!("../tests/fixtures/nightwave_acts.json");
    const DICTIONARY: &str = include_str!("../tests/fixtures/nightwave_dictionary.json");

    #[test]
    fn act_by_name() {
        let acts = NightwaveActs::from_json(EXPORT, DICTIONARY).unwrap();
        let trampoline = acts.get("SeasonDailyBulletJump").unwrap();

        assert_eq!(trampoline.title, "Trampoline");
        assert_eq!(trampoline.description_template, "Bullet Jump |COUNT| times");
        assert_eq!(trampoline.required, 150);
    }

    #[test]
    fn untranslated_act_left_out() {
        let export = r#"{"challenges":{"/Lotus/Types/Challenges/Seasons/Daily/SeasonDailyNew":{"name":"/Lotus/Language/New_Name","description":"/Lotus/Language/New_Description","required":2}}}"#;
        let acts =
            NightwaveActs::from_json(export, r#"{"/Lotus/Language/New_Name":"New"}"#).unwrap();

        assert_eq!(acts.get("SeasonDailyNew"), None);
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(NightwaveActs::from_json("{", "{}").is_err());
        assert!(NightwaveActs::from_json(r#"{"challenges":{}}"#, "[").is_err());
    }
}
