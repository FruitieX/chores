use crate::bedtime::{Event, Schedule, Swap};
use crate::error::{ApiError, Result};
use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};
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
pub struct Household {
    pub revision: u64,
    pub settings: Settings,
    pub chores: Vec<Chore>,
    pub completions: Vec<Completion>,
    pub bedtime_events: Vec<Event>,
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
    UndoSwap {
        id: String,
    },
    RestartBedtime {
        date: String,
        #[serde(rename = "firstPerson")]
        first_person: String,
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
        || !["Everyday", "Laundry", "Maintenance"].contains(&chore.category.as_str())
        || chore.points > 1000
        || chore.interval_days.is_some_and(|v| v == 0 || v > 3650)
        || chore.id == "bedtime"
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
            bedtime_events: vec![],
        }
    }
    pub fn schedule(&self) -> Result<Schedule> {
        Schedule::replay(&self.settings, &self.bedtime_events)
    }
    #[cfg(test)]
    pub fn assignee(&self, day: &str) -> Result<String> {
        self.schedule()?.assignee(day)
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
                if settings.bedtime_start != self.settings.bedtime_start
                    || settings.bedtime_first != self.settings.bedtime_first
                {
                    return Err(ApiError::bad(
                        "Change the bedtime pattern from a date in the calendar.",
                    ));
                }
                self.settings = settings;
            }
            Operation::Swap { first, second } => {
                if date(&first)? < today || date(&second)? < today {
                    return Err(ApiError::bad("Choose two upcoming nights, today or later."));
                }
                let mut schedule = self.schedule()?;
                let event = Event::Swap {
                    swap: Swap {
                        id: Uuid::new_v4().to_string(),
                        first_person: schedule.assignee(&first)?,
                        second_person: schedule.assignee(&second)?,
                        first,
                        second,
                        actor: actor.into(),
                        created_at: now.into(),
                    },
                };
                schedule.apply(&event)?;
                self.bedtime_events.push(event);
            }
            Operation::UndoSwap { id } => {
                let mut schedule = self.schedule()?;
                let view = schedule
                    .swaps
                    .iter()
                    .find(|s| s.swap.id == id)
                    .ok_or_else(|| ApiError::bad("This swap could not be found."))?;
                if let Some(reason) = &view.undo_blocked {
                    return Err(ApiError::bad(reason));
                }
                let event = Event::UndoSwap {
                    swap_id: id,
                    actor: actor.into(),
                    created_at: now.into(),
                };
                schedule.apply(&event)?;
                self.bedtime_events.push(event);
            }
            Operation::RestartBedtime {
                date: day,
                first_person,
            } => {
                let mut schedule = self.schedule()?;
                let event = Event::Restart {
                    id: Uuid::new_v4().to_string(),
                    date: day,
                    first_person,
                    actor: actor.into(),
                    created_at: now.into(),
                };
                schedule.apply(&event)?;
                self.bedtime_events.push(event);
            }
        }
        self.revision += 1;
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        valid_settings(&self.settings)?;
        if self.chores.len() > 200
            || self.completions.len() > 100000
            || self.bedtime_events.len() > 100000
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
        let mut entry_ids = std::collections::HashSet::new();
        for c in &self.completions {
            date(&c.date)?;
            if !entry_ids.insert(&c.id)
                || !text(&c.id, 80)
                || !text(&c.name, 80)
                || !["a", "b"].contains(&c.person.as_str())
                || c.points > 1000
                || c.note.chars().count() > 500
                || (c.chore_id != "bedtime" && !self.chores.iter().any(|t| t.id == c.chore_id))
            {
                return Err(ApiError::bad("Invalid completion in backup."));
            }
        }
        self.schedule()?;
        Ok(())
    }
}

pub fn migrate_legacy(mut value: serde_json::Value) -> Result<Household> {
    let swaps: Vec<Swap> = serde_json::from_value(
        value
            .get("swaps")
            .cloned()
            .ok_or_else(|| ApiError::bad("Missing legacy calendar."))?,
    )?;
    let overrides: std::collections::BTreeMap<String, String> = serde_json::from_value(
        value
            .get("overrides")
            .cloned()
            .ok_or_else(|| ApiError::bad("Missing legacy assignments."))?,
    )?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ApiError::bad("Invalid household backup."))?;
    object.remove("swaps");
    object.remove("overrides");
    let chores = object
        .get_mut("chores")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| ApiError::bad("Missing chores."))?;
    chores.retain(|c| c["id"] != "bedtime");
    object.insert(
        "bedtimeEvents".into(),
        serde_json::to_value(
            swaps
                .into_iter()
                .map(|swap| Event::Swap { swap })
                .collect::<Vec<_>>(),
        )?,
    );
    let h: Household = serde_json::from_value(value)?;
    h.validate()?;
    if h.schedule()?.overrides != overrides {
        return Err(ApiError::bad(
            "Legacy assignments do not match paired swaps.",
        ));
    }
    Ok(h)
}
#[cfg(test)]
mod tests;
