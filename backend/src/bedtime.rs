use crate::{
    domain::{Settings, date},
    error::{ApiError, Result},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Swap {
    pub id: String,
    pub first: String,
    pub second: String,
    pub first_person: String,
    pub second_person: String,
    pub actor: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Event {
    Swap {
        swap: Swap,
    },
    UndoSwap {
        #[serde(rename = "swapId")]
        swap_id: String,
        actor: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
    Restart {
        id: String,
        date: String,
        #[serde(rename = "firstPerson")]
        first_person: String,
        actor: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    pub date: String,
    pub person: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapView {
    #[serde(flatten)]
    pub swap: Swap,
    pub undone: bool,
    pub superseded: bool,
    pub undo_blocked: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Schedule {
    pub starts: Vec<Start>,
    pub overrides: BTreeMap<String, String>,
    pub swaps: Vec<SwapView>,
}
fn person(value: &str) -> Result<()> {
    if ["a", "b"].contains(&value) {
        Ok(())
    } else {
        Err(ApiError::bad("Choose one of the two adults."))
    }
}
impl Schedule {
    pub fn new(settings: &Settings) -> Self {
        Self {
            starts: vec![Start {
                date: settings.bedtime_start.clone(),
                person: settings.bedtime_first.clone(),
            }],
            overrides: BTreeMap::new(),
            swaps: vec![],
        }
    }
    pub fn baseline(&self, day: &str) -> Result<String> {
        let d = date(day)?;
        let start = self
            .starts
            .iter()
            .rev()
            .find(|s| s.date.as_str() <= day)
            .ok_or_else(|| ApiError::bad("This date is before the bedtime schedule starts."))?;
        let even = d.signed_duration_since(date(&start.date)?).num_days() % 2 == 0;
        Ok(if even {
            start.person.clone()
        } else if start.person == "a" {
            "b".into()
        } else {
            "a".into()
        })
    }
    pub fn assignee(&self, day: &str) -> Result<String> {
        date(day)?;
        self.overrides
            .get(day)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| self.baseline(day))
    }
    fn assign(&mut self, day: &str, person: &str) -> Result<()> {
        if self.baseline(day)? == person {
            self.overrides.remove(day);
        } else {
            self.overrides.insert(day.into(), person.into());
        }
        Ok(())
    }
    fn undo_reason(&self, index: usize) -> Option<String> {
        let s = &self.swaps[index];
        if s.undone {
            return Some("Already undone".into());
        }
        if s.superseded {
            return Some("Replaced by a new pattern".into());
        }
        if self.swaps[index + 1..].iter().any(|later| {
            !later.undone
                && !later.superseded
                && [&later.swap.first, &later.swap.second]
                    .iter()
                    .any(|d| **d == s.swap.first || **d == s.swap.second)
        }) {
            return Some("Undo the later overlapping swap first".into());
        }
        if self.assignee(&s.swap.first).ok().as_deref() != Some(s.swap.second_person.as_str())
            || self.assignee(&s.swap.second).ok().as_deref() != Some(s.swap.first_person.as_str())
        {
            return Some("Changed by a later calendar update".into());
        }
        None
    }
    pub fn apply(&mut self, event: &Event) -> Result<()> {
        match event {
            Event::Swap { swap: s } => {
                person(&s.actor)?;
                person(&s.first_person)?;
                person(&s.second_person)?;
                if s.id.is_empty()
                    || s.id.len() > 80
                    || self.swaps.iter().any(|old| old.swap.id == s.id)
                    || s.first == s.second
                    || s.first_person == s.second_person
                    || self.assignee(&s.first)? != s.first_person
                    || self.assignee(&s.second)? != s.second_person
                {
                    return Err(ApiError::bad(
                        "Choose one night for each person, so the swap keeps things equal.",
                    ));
                }
                self.assign(&s.first, &s.second_person)?;
                self.assign(&s.second, &s.first_person)?;
                self.swaps.push(SwapView {
                    swap: s.clone(),
                    undone: false,
                    superseded: false,
                    undo_blocked: None,
                });
            }
            Event::UndoSwap { swap_id, actor, .. } => {
                person(actor)?;
                let index = self
                    .swaps
                    .iter()
                    .position(|s| s.swap.id == *swap_id)
                    .ok_or_else(|| ApiError::bad("This swap could not be found."))?;
                if let Some(reason) = self.undo_reason(index) {
                    return Err(ApiError::bad(reason));
                }
                let s = self.swaps[index].swap.clone();
                if self.assignee(&s.first)? != s.second_person
                    || self.assignee(&s.second)? != s.first_person
                {
                    return Err(ApiError::conflict());
                }
                self.assign(&s.first, &s.first_person)?;
                self.assign(&s.second, &s.second_person)?;
                self.swaps[index].undone = true;
            }
            Event::Restart {
                date: day,
                first_person,
                actor,
                ..
            } => {
                date(day)?;
                person(first_person)?;
                person(actor)?;
                self.starts.retain(|s| s.date < *day);
                self.starts.push(Start {
                    date: day.clone(),
                    person: first_person.clone(),
                });
                self.overrides.retain(|d, _| d < day);
                for swap in &mut self.swaps {
                    if !swap.undone && (swap.swap.first >= *day || swap.swap.second >= *day) {
                        swap.superseded = true;
                    }
                }
            }
        }
        Ok(())
    }
    pub fn replay(settings: &Settings, events: &[Event]) -> Result<Self> {
        if events.len() > 100000 {
            return Err(ApiError::bad("Too many calendar changes."));
        }
        let mut result = Self::new(settings);
        let mut reset_ids = HashSet::new();
        for event in events {
            if let Event::Restart { id, .. } = event
                && (id.is_empty() || id.len() > 80 || !reset_ids.insert(id))
            {
                return Err(ApiError::bad("Invalid calendar change."));
            }
            result.apply(event)?;
        }
        for i in 0..result.swaps.len() {
            result.swaps[i].undo_blocked = result.undo_reason(i);
        }
        Ok(result)
    }
}
