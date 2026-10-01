use crate::error::{ApiError, Result};
use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Chore {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub category: String,
    pub points: u32,
    pub interval_days: Option<u32>,
    pub archived: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub household_name: String,
    pub names: [String; 2],
    pub balance_days: u32,
    pub bedtime_start: String,
    pub bedtime_first: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Completion {
    pub id: String,
    pub chore_id: String,
    pub name: String,
    pub icon: String,
    pub person: String,
    pub points: u32,
    pub date: String,
    pub note: String,
    pub created_at: String,
    pub undone: bool,
}
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Household {
    pub revision: u64,
    pub settings: Settings,
    pub chores: Vec<Chore>,
    pub completions: Vec<Completion>,
    pub overrides: BTreeMap<String, String>,
    pub swaps: Vec<Swap>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Operation {
    Complete {
        #[serde(rename = "choreId")]
        chore_id: String,
        #[serde(rename = "expectedPoints")]
        expected_points: u32,
        date: String,
        note: String,
    },
    Undo {
        id: String,
    },
    SaveChore {
        chore: Chore,
        expected: Option<Chore>,
    },
    SaveSettings {
        settings: Settings,
        expected: Settings,
    },
    Swap {
        first: String,
        second: String,
    },
}
pub fn date(value: &str) -> Result<NaiveDate> {
    let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| ApiError::bad("Choose a valid date."))?;
    if parsed.to_string() != value {
        return Err(ApiError::bad("Use YYYY-MM-DD dates."));
    }
    Ok(parsed)
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}
pub fn valid_chore(chore: &Chore) -> Result<()> {
    if !text(&chore.id, 80)
        || !text(&chore.name, 80)
        || !text(&chore.icon, 30)
        || !["Everyday", "Laundry", "Maintenance", "Bedtime"].contains(&chore.category.as_str())
        || chore.points > 1000
        || chore.interval_days.is_some_and(|v| v == 0 || v > 3650)
        || (chore.id == "bedtime"
            && (chore.archived || chore.category != "Bedtime" || chore.interval_days.is_some()))
        || (chore.id != "bedtime" && chore.category == "Bedtime")
    {
        return Err(ApiError::bad(
            "Check the chore name, category, points (0–1000), and repeat interval (1–3650 days).",
        ));
    }
    Ok(())
}
pub fn valid_settings(settings: &Settings) -> Result<()> {
    if !text(&settings.household_name, 80)
        || settings.names.iter().any(|n| !text(n, 40))
        || settings.balance_days > 3650
        || !["a", "b"].contains(&settings.bedtime_first.as_str())
    {
        return Err(ApiError::bad(
            "Check household details and balance window (0–3650 days).",
        ));
    }
    date(&settings.bedtime_start)?;
    Ok(())
}
impl Household {
    pub fn new(settings: Settings) -> Self {
        let definitions = [
            (
                "dishwasher",
                "Run dishwasher",
                "dishwasher",
                "Everyday",
                2,
                None,
            ),
            ("washer", "Run clothes washer", "washer", "Laundry", 2, None),
            ("hang", "Hang laundry to dry", "hang", "Laundry", 4, None),
            (
                "fold",
                "Fold & put away laundry",
                "fold",
                "Laundry",
                5,
                None,
            ),
            ("tidy", "Tidy up", "tidy", "Everyday", 4, None),
            (
                "bedtime",
                "Put kids to sleep",
                "bedtime",
                "Bedtime",
                5,
                None,
            ),
            (
                "robot",
                "Maintain robot vacuum",
                "robot",
                "Maintenance",
                3,
                Some(7),
            ),
            (
                "hvac",
                "Change HVAC filters",
                "filter",
                "Maintenance",
                5,
                Some(90),
            ),
            (
                "drain",
                "Clean drain filters",
                "drain",
                "Maintenance",
                3,
                Some(30),
            ),
        ];
        Self {
            revision: 0,
            settings,
            chores: definitions
                .into_iter()
                .map(|(id, name, icon, category, points, interval_days)| Chore {
                    id: id.into(),
                    name: name.into(),
                    icon: icon.into(),
                    category: category.into(),
                    points,
                    interval_days,
                    archived: false,
                })
                .collect(),
            completions: vec![],
            overrides: BTreeMap::new(),
            swaps: vec![],
        }
    }
    pub fn baseline(&self, value: &str) -> Result<String> {
        let diff = date(value)?
            .signed_duration_since(date(&self.settings.bedtime_start)?)
            .num_days();
        if diff < 0 {
            return Err(ApiError::bad(
                "This date is before the bedtime schedule starts.",
            ));
        }
        Ok(if diff % 2 == 0 {
            self.settings.bedtime_first.clone()
        } else if self.settings.bedtime_first == "a" {
            "b".into()
        } else {
            "a".into()
        })
    }
    pub fn assignee(&self, value: &str) -> Result<String> {
        self.overrides
            .get(value)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| self.baseline(value))
    }
    pub fn apply(&mut self, op: Operation, actor: &str, today: NaiveDate, now: &str) -> Result<()> {
        if !["a", "b"].contains(&actor) {
            return Err(ApiError::unauthorized());
        }
        match op {
            Operation::Complete {
                chore_id,
                expected_points,
                date: day,
                note,
            } => {
                let completed = date(&day)?;
                if completed > today
                    || completed < today.checked_sub_days(Days::new(3650)).unwrap_or(today)
                    || note.chars().count() > 500
                {
                    return Err(ApiError::bad(
                        "Choose today or a past date, and a note under 500 characters.",
                    ));
                }
                let chore = self
                    .chores
                    .iter()
                    .find(|c| c.id == chore_id && !c.archived)
                    .ok_or_else(|| ApiError::bad("This chore is no longer available."))?;
                if chore.points != expected_points {
                    return Err(ApiError::conflict());
                }
                if chore_id == "bedtime" {
                    if self
                        .completions
                        .iter()
                        .any(|c| c.chore_id == "bedtime" && c.date == day && !c.undone)
                    {
                        return Err(ApiError::bad("Bedtime is already recorded for this date."));
                    }
                    if self.assignee(&day)? != actor {
                        return Err(ApiError::bad(
                            "This is your partner’s bedtime turn. Swap the dates first.",
                        ));
                    }
                }
                self.completions.push(Completion {
                    id: Uuid::new_v4().to_string(),
                    chore_id: chore.id.clone(),
                    name: chore.name.clone(),
                    icon: chore.icon.clone(),
                    person: actor.into(),
                    points: chore.points,
                    date: day,
                    note: note.trim().into(),
                    created_at: now.into(),
                    undone: false,
                });
            }
            Operation::Undo { id } => {
                let entry = self
                    .completions
                    .iter_mut()
                    .find(|c| c.id == id && !c.undone)
                    .ok_or_else(|| ApiError::bad("This completion was already removed."))?;
                if entry.person != actor {
                    return Err(ApiError::bad(
                        "Only the person who recorded this chore can undo it.",
                    ));
                }
                entry.undone = true;
            }
            Operation::SaveChore { chore, expected } => {
                if self.chores.iter().find(|c| c.id == chore.id) != expected.as_ref() {
                    return Err(ApiError::conflict());
                }
                valid_chore(&chore)?;
                if let Some(entry) = self.chores.iter_mut().find(|c| c.id == chore.id) {
                    *entry = chore;
                } else {
                    if self.chores.len() >= 200 {
                        return Err(ApiError::bad("The household can have up to 200 chores."));
                    }
                    self.chores.push(chore);
                }
            }
            Operation::SaveSettings { settings, expected } => {
                if self.settings != expected {
                    return Err(ApiError::conflict());
                }
                valid_settings(&settings)?;
                if (settings.bedtime_start != self.settings.bedtime_start
                    || settings.bedtime_first != self.settings.bedtime_first)
                    && (!self.swaps.is_empty()
                        || self.completions.iter().any(|c| c.chore_id == "bedtime"))
                {
                    return Err(ApiError::bad(
                        "The bedtime starting pattern is locked after the first swap or completion. Swap individual dates instead.",
                    ));
                }
                self.settings = settings;
            }
            Operation::Swap { first, second } => {
                if first == second || date(&first)? < today || date(&second)? < today {
                    return Err(ApiError::bad("Choose two different dates, today or later."));
                }
                if self.completions.iter().any(|c| {
                    c.chore_id == "bedtime" && !c.undone && (c.date == first || c.date == second)
                }) {
                    return Err(ApiError::bad(
                        "A completed bedtime cannot be swapped. Undo the completion first.",
                    ));
                }
                let a = self.assignee(&first)?;
                let b = self.assignee(&second)?;
                if a == b {
                    return Err(ApiError::bad(
                        "Choose one night for each person, so the swap keeps things equal.",
                    ));
                }
                self.overrides.insert(first.clone(), b.clone());
                self.overrides.insert(second.clone(), a.clone());
                if self.baseline(&first)? == b {
                    self.overrides.remove(&first);
                }
                if self.baseline(&second)? == a {
                    self.overrides.remove(&second);
                }
                self.swaps.push(Swap {
                    id: Uuid::new_v4().to_string(),
                    first,
                    second,
                    first_person: a,
                    second_person: b,
                    actor: actor.into(),
                    created_at: now.into(),
                });
            }
        }
        self.revision += 1;
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        valid_settings(&self.settings)?;
        if self.chores.len() > 200
            || self.completions.len() > 100000
            || self.swaps.len() > 100000
            || self.overrides.len() > 100000
        {
            return Err(ApiError::bad("Backup exceeds household limits."));
        }
        let mut ids = std::collections::HashSet::new();
        for chore in &self.chores {
            valid_chore(chore)?;
            if !ids.insert(&chore.id) {
                return Err(ApiError::bad("Duplicate chore."));
            }
        }
        if !self.chores.iter().any(|c| c.id == "bedtime") {
            return Err(ApiError::bad("Bedtime chore is required."));
        }
        let mut days = std::collections::HashSet::new();
        let mut entry_ids = std::collections::HashSet::new();
        for c in &self.completions {
            date(&c.date)?;
            if !entry_ids.insert(&c.id)
                || !text(&c.id, 80)
                || !text(&c.name, 80)
                || !["a", "b"].contains(&c.person.as_str())
                || c.points > 1000
                || c.note.chars().count() > 500
                || !self.chores.iter().any(|t| t.id == c.chore_id)
                || (c.chore_id == "bedtime"
                    && !c.undone
                    && (!days.insert(&c.date) || self.assignee(&c.date)? != c.person))
            {
                return Err(ApiError::bad("Invalid completion in backup."));
            }
        }
        // Replay all paired swaps to prove imported assignments preserve the alternation's totals.
        let mut replay = self.clone();
        replay.overrides.clear();
        for swap in &self.swaps {
            if swap.first == swap.second
                || !["a", "b"].contains(&swap.actor.as_str())
                || replay.assignee(&swap.first)? != swap.first_person
                || replay.assignee(&swap.second)? != swap.second_person
                || swap.first_person == swap.second_person
            {
                return Err(ApiError::bad("Invalid paired swap in backup."));
            }
            replay
                .overrides
                .insert(swap.first.clone(), swap.second_person.clone());
            replay
                .overrides
                .insert(swap.second.clone(), swap.first_person.clone());
            if replay.baseline(&swap.first)? == swap.second_person {
                replay.overrides.remove(&swap.first);
            }
            if replay.baseline(&swap.second)? == swap.first_person {
                replay.overrides.remove(&swap.second);
            }
        }
        if replay.overrides != self.overrides {
            return Err(ApiError::bad(
                "Backup assignments do not match paired swaps.",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn household() -> Household {
        Household::new(Settings {
            household_name: "Home".into(),
            names: ["A".into(), "B".into()],
            balance_days: 30,
            bedtime_start: "2026-10-01".into(),
            bedtime_first: "a".into(),
        })
    }
    fn today() -> NaiveDate {
        date("2026-10-01").unwrap()
    }
    #[test]
    fn swap_conserves_nights_and_can_reverse() {
        let mut h = household();
        let count = |h: &Household| {
            (1..=30)
                .filter(|d| h.assignee(&format!("2026-10-{d:02}")).unwrap() == "a")
                .count()
        };
        assert_eq!(count(&h), 15);
        h.apply(
            Operation::Swap {
                first: "2026-10-01".into(),
                second: "2026-10-04".into(),
            },
            "a",
            today(),
            "now",
        )
        .unwrap();
        assert_eq!(count(&h), 15);
        assert_eq!(h.assignee("2026-10-01").unwrap(), "b");
        h.validate().unwrap();
        h.apply(
            Operation::Swap {
                first: "2026-10-01".into(),
                second: "2026-10-04".into(),
            },
            "b",
            today(),
            "now",
        )
        .unwrap();
        assert!(h.overrides.is_empty());
        h.validate().unwrap();
    }
    #[test]
    fn rejects_unfair_past_and_completed_swaps() {
        let mut h = household();
        for (first, second) in [
            ("2026-10-01", "2026-10-03"),
            ("2026-09-30", "2026-10-01"),
            ("2026-10-01", "2026-10-01"),
        ] {
            assert!(
                h.apply(
                    Operation::Swap {
                        first: first.into(),
                        second: second.into()
                    },
                    "a",
                    today(),
                    "now"
                )
                .is_err()
            );
        }
        h.apply(
            Operation::Complete {
                chore_id: "bedtime".into(),
                expected_points: 5,
                date: "2026-10-01".into(),
                note: "".into(),
            },
            "a",
            today(),
            "now",
        )
        .unwrap();
        assert!(
            h.apply(
                Operation::Swap {
                    first: "2026-10-01".into(),
                    second: "2026-10-02".into()
                },
                "a",
                today(),
                "now"
            )
            .is_err()
        );
    }
    #[test]
    fn historical_points_undo_and_bedtime_uniqueness() {
        let mut h = household();
        let op = Operation::Complete {
            chore_id: "bedtime".into(),
            expected_points: 5,
            date: "2026-10-01".into(),
            note: "".into(),
        };
        assert!(h.apply(op.clone(), "b", today(), "now").is_err());
        h.apply(op.clone(), "a", today(), "now").unwrap();
        assert!(h.apply(op.clone(), "a", today(), "now").is_err());
        let id = h.completions[0].id.clone();
        assert!(
            h.apply(Operation::Undo { id: id.clone() }, "b", today(), "now")
                .is_err()
        );
        let mut chore = h.chores.iter().find(|c| c.id == "bedtime").unwrap().clone();
        chore.points = 20;
        h.apply(
            Operation::SaveChore {
                expected: Some(h.chores.iter().find(|c| c.id == "bedtime").unwrap().clone()),
                chore,
            },
            "a",
            today(),
            "now",
        )
        .unwrap();
        assert_eq!(h.completions[0].points, 5);
        h.apply(Operation::Undo { id }, "a", today(), "now")
            .unwrap();
        let op = Operation::Complete {
            chore_id: "bedtime".into(),
            expected_points: 20,
            date: "2026-10-01".into(),
            note: "".into(),
        };
        h.apply(op, "a", today(), "now").unwrap();
        assert_eq!(h.completions[1].points, 20);
    }
    #[test]
    fn stale_edit_values_cannot_overwrite_new_configuration() {
        let mut h = household();
        let original = h.chores[0].clone();
        let mut newer = original.clone();
        newer.points = 9;
        h.apply(
            Operation::SaveChore {
                chore: newer.clone(),
                expected: Some(original.clone()),
            },
            "b",
            today(),
            "now",
        )
        .unwrap();
        let mut stale = original.clone();
        stale.name = "Old editor".into();
        assert!(
            h.apply(
                Operation::SaveChore {
                    chore: stale,
                    expected: Some(original)
                },
                "a",
                today(),
                "now"
            )
            .is_err()
        );
        assert_eq!(h.chores[0], newer);
        assert!(
            h.apply(
                Operation::Complete {
                    chore_id: "dishwasher".into(),
                    expected_points: 2,
                    date: "2026-10-01".into(),
                    note: "".into()
                },
                "a",
                today(),
                "now"
            )
            .is_err()
        );
        let old_settings = h.settings.clone();
        let mut changed = old_settings.clone();
        changed.balance_days = 7;
        h.apply(
            Operation::SaveSettings {
                settings: changed.clone(),
                expected: old_settings.clone(),
            },
            "b",
            today(),
            "now",
        )
        .unwrap();
        assert!(
            h.apply(
                Operation::SaveSettings {
                    settings: old_settings.clone(),
                    expected: old_settings
                },
                "a",
                today(),
                "now"
            )
            .is_err()
        );
        assert_eq!(h.settings, changed);
    }
    #[test]
    fn locks_anchor_and_rejects_forged_backup() {
        let mut h = household();
        h.apply(
            Operation::Swap {
                first: "2026-10-01".into(),
                second: "2026-10-02".into(),
            },
            "a",
            today(),
            "now",
        )
        .unwrap();
        let mut settings = h.settings.clone();
        settings.bedtime_first = "b".into();
        assert!(
            h.apply(
                Operation::SaveSettings {
                    expected: h.settings.clone(),
                    settings
                },
                "a",
                today(),
                "now"
            )
            .is_err()
        );
        h.overrides.insert("2026-10-03".into(), "b".into());
        assert!(h.validate().is_err());
    }
}
