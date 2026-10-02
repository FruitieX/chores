use super::*;
fn household() -> Household {
    Household::new(Settings {
        household_name: "Home".into(),
        names: ["Alex".into(), "Sam".into()],
        balance_days: 30,
        bedtime_start: "2026-10-01".into(),
        bedtime_first: "a".into(),
    })
}
fn today() -> NaiveDate {
    date("2026-10-01").unwrap()
}
fn apply(h: &mut Household, op: Operation) {
    h.apply(op, "a", today(), "2026-10-01T10:00:00Z").unwrap()
}
fn swap(h: &mut Household, a: &str, b: &str) -> String {
    apply(
        h,
        Operation::Swap {
            first: a.into(),
            second: b.into(),
        },
    );
    h.schedule().unwrap().swaps.last().unwrap().swap.id.clone()
}
#[test]
fn paired_swap_and_undo_preserve_turn_totals() {
    let mut h = household();
    let count = |h: &Household| {
        (1..=30)
            .filter(|d| h.assignee(&format!("2026-10-{d:02}")).unwrap() == "a")
            .count()
    };
    assert_eq!(count(&h), 15);
    let id = swap(&mut h, "2026-10-01", "2026-10-04");
    assert_eq!(count(&h), 15);
    assert_eq!(h.assignee("2026-10-01").unwrap(), "b");
    apply(&mut h, Operation::UndoSwap { id: id.clone() });
    assert_eq!(count(&h), 15);
    assert!(h.schedule().unwrap().overrides.is_empty());
    assert!(h.schedule().unwrap().swaps[0].undone);
    assert!(
        h.apply(Operation::UndoSwap { id }, "b", today(), "now")
            .is_err()
    );
    h.validate().unwrap();
}
#[test]
fn undo_respects_later_overlapping_changes() {
    let mut h = household();
    let first = swap(&mut h, "2026-10-01", "2026-10-02");
    let later = swap(&mut h, "2026-10-01", "2026-10-03");
    assert!(
        h.apply(
            Operation::UndoSwap { id: first.clone() },
            "b",
            today(),
            "now"
        )
        .is_err()
    );
    apply(&mut h, Operation::UndoSwap { id: later });
    apply(&mut h, Operation::UndoSwap { id: first });
    assert!(h.schedule().unwrap().overrides.is_empty());
    h.validate().unwrap();
}
#[test]
fn rejects_same_person_and_past_swaps_but_allows_undo_after_the_nights() {
    let mut h = household();
    for (a, b) in [
        ("2026-10-01", "2026-10-03"),
        ("2026-09-30", "2026-10-01"),
        ("2026-10-01", "2026-10-01"),
    ] {
        assert!(
            h.apply(
                Operation::Swap {
                    first: a.into(),
                    second: b.into()
                },
                "a",
                today(),
                "now"
            )
            .is_err()
        );
    }
    let id = swap(&mut h, "2026-10-01", "2026-10-02");
    h.apply(
        Operation::UndoSwap { id },
        "b",
        date("2026-10-03").unwrap(),
        "now",
    )
    .unwrap();
    assert!(h.schedule().unwrap().overrides.is_empty());
}
#[test]
fn restarting_from_a_day_preserves_every_earlier_assignment() {
    let mut h = household();
    let old = swap(&mut h, "2026-10-01", "2026-10-06");
    let future = swap(&mut h, "2026-10-09", "2026-10-10");
    let before = (1..=4)
        .map(|d| h.assignee(&format!("2026-10-{d:02}")).unwrap())
        .collect::<Vec<_>>();
    apply(
        &mut h,
        Operation::RestartBedtime {
            date: "2026-10-05".into(),
            first_person: "b".into(),
        },
    );
    assert_eq!(
        before,
        (1..=4)
            .map(|d| h.assignee(&format!("2026-10-{d:02}")).unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(h.assignee("2026-10-05").unwrap(), "b");
    assert_eq!(h.assignee("2026-10-06").unwrap(), "a");
    assert_eq!(h.assignee("2026-10-09").unwrap(), "b");
    for id in [old, future] {
        assert!(
            h.apply(Operation::UndoSwap { id }, "a", today(), "now")
                .is_err()
        );
    }
    h.validate().unwrap();
}
#[test]
fn multiple_restarts_and_new_swaps_replay_in_backups() {
    let mut h = household();
    apply(
        &mut h,
        Operation::RestartBedtime {
            date: "2026-10-07".into(),
            first_person: "b".into(),
        },
    );
    apply(
        &mut h,
        Operation::RestartBedtime {
            date: "2026-10-11".into(),
            first_person: "a".into(),
        },
    );
    let before = h.assignee("2026-10-08").unwrap();
    apply(
        &mut h,
        Operation::RestartBedtime {
            date: "2026-10-09".into(),
            first_person: "b".into(),
        },
    );
    assert_eq!(h.assignee("2026-10-08").unwrap(), before);
    assert_eq!(h.assignee("2026-10-11").unwrap(), "b");
    let id = swap(&mut h, "2026-10-09", "2026-10-10");
    apply(&mut h, Operation::UndoSwap { id });
    let restored: Household = serde_json::from_str(&serde_json::to_string(&h).unwrap()).unwrap();
    restored.validate().unwrap();
    for d in 1..=31 {
        assert_eq!(
            h.assignee(&format!("2026-10-{d:02}")).unwrap(),
            restored.assignee(&format!("2026-10-{d:02}")).unwrap()
        );
    }
}
#[test]
fn a_restart_can_extend_the_calendar_without_rewriting_earlier_days() {
    let mut h = household();
    apply(
        &mut h,
        Operation::RestartBedtime {
            date: "2026-09-25".into(),
            first_person: "b".into(),
        },
    );
    assert!(h.assignee("2026-09-24").is_err());
    assert_eq!(h.assignee("2026-09-25").unwrap(), "b");
    assert_eq!(h.assignee("2026-10-01").unwrap(), "b");
    h.validate().unwrap();
}
#[test]
fn bedtime_is_not_a_chore_and_saved_points_do_not_change() {
    let mut h = household();
    assert!(!h.chores.iter().any(|c| c.id == "bedtime"));
    assert!(
        h.apply(
            Operation::Complete {
                chore_id: "bedtime".into(),
                expected_points: 5,
                date: "2026-10-01".into(),
                note: "".into()
            },
            "a",
            today(),
            "now"
        )
        .is_err()
    );
    apply(
        &mut h,
        Operation::Complete {
            chore_id: "dishwasher".into(),
            expected_points: 2,
            date: "2026-10-01".into(),
            note: "".into(),
        },
    );
    let original = h.chores[0].clone();
    let mut changed = original.clone();
    changed.points = 9;
    apply(
        &mut h,
        Operation::SaveChore {
            chore: changed,
            expected: Some(original),
        },
    );
    assert_eq!(h.completions[0].points, 2);
    let id = h.completions[0].id.clone();
    assert!(
        h.apply(Operation::Undo { id: id.clone() }, "b", today(), "now")
            .is_err()
    );
    apply(&mut h, Operation::Undo { id });
    assert_eq!(h.completions[0].points, 2);
    assert!(h.completions[0].undone);
}
#[test]
fn stale_edit_values_cannot_overwrite_new_configuration() {
    let mut h = household();
    let original = h.chores[0].clone();
    let mut newer = original.clone();
    newer.points = 9;
    apply(
        &mut h,
        Operation::SaveChore {
            chore: newer.clone(),
            expected: Some(original.clone()),
        },
    );
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
    let old = h.settings.clone();
    let mut changed = old.clone();
    changed.balance_days = 7;
    apply(
        &mut h,
        Operation::SaveSettings {
            settings: changed.clone(),
            expected: old.clone(),
        },
    );
    assert!(
        h.apply(
            Operation::SaveSettings {
                settings: old.clone(),
                expected: old
            },
            "b",
            today(),
            "now"
        )
        .is_err()
    );
    assert_eq!(h.settings, changed);
}
#[test]
fn legacy_migration_preserves_history_and_calendar() {
    let mut h = household();
    swap(&mut h, "2026-10-01", "2026-10-02");
    let mut legacy = serde_json::to_value(&h).unwrap();
    let object = legacy.as_object_mut().unwrap();
    object.remove("bedtimeEvents");
    object.insert(
        "overrides".into(),
        serde_json::to_value(h.schedule().unwrap().overrides).unwrap(),
    );
    object.insert(
        "swaps".into(),
        serde_json::to_value(
            h.schedule()
                .unwrap()
                .swaps
                .iter()
                .map(|s| s.swap.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap(),
    );
    object["chores"].as_array_mut().unwrap().push(serde_json::json!({"id":"bedtime","name":"Put kids to sleep","category":"Bedtime","points":5,"icon":"bedtime","intervalDays":null,"archived":false}));
    object["completions"].as_array_mut().unwrap().push(serde_json::json!({"id":"legacy-night","choreId":"bedtime","name":"Put kids to sleep","icon":"bedtime","person":"b","points":5,"date":"2026-10-01","note":"saved","createdAt":"now","undone":false}));
    let mut migrated = migrate_legacy(legacy.clone()).unwrap();
    assert_eq!(migrated.assignee("2026-10-01").unwrap(), "b");
    assert_eq!(migrated.completions[0].points, 5);
    assert!(!migrated.chores.iter().any(|c| c.id == "bedtime"));
    let id = migrated.schedule().unwrap().swaps[0].swap.id.clone();
    apply(&mut migrated, Operation::UndoSwap { id });
    migrated.validate().unwrap();
    legacy["overrides"]["2026-10-03"] = serde_json::json!("b");
    assert!(migrate_legacy(legacy).is_err());
}

#[test]
fn a_crossing_restart_reports_why_an_earlier_overlapping_swap_cannot_be_undone() {
    let mut h = household();
    let first = swap(&mut h, "2026-10-01", "2026-10-02");
    swap(&mut h, "2026-10-02", "2026-10-04");
    let before = h.assignee("2026-10-02").unwrap();
    apply(
        &mut h,
        Operation::RestartBedtime {
            date: "2026-10-04".into(),
            first_person: "b".into(),
        },
    );
    assert_eq!(h.assignee("2026-10-02").unwrap(), before);
    let schedule = h.schedule().unwrap();
    assert_eq!(
        schedule.swaps[0].undo_blocked.as_deref(),
        Some("Changed by a later calendar update")
    );
    assert!(
        h.apply(Operation::UndoSwap { id: first }, "a", today(), "now")
            .is_err()
    );
    h.validate().unwrap();
}
