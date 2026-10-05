mod support;
use codegrid_level_core::{
    scene_evaluate::{VisibleSceneFailure, VisibleSceneOutcome},
    scene_feedback::*,
    scene_protocol::SceneFailure,
    scenes::SceneKind,
    EvaluationMode,
};
use support::n;
fn draft(payload: SceneEventPayload) -> SceneEventDraft {
    SceneEventDraft {
        source_index: 0,
        scene_type: SceneKind::Robot,
        tick: Some(1),
        frame_index: Some(1),
        actor: Some(SceneActor::A),
        payload,
    }
}
fn wait() -> SceneEventDraft {
    draft(SceneEventPayload::ActionApplied {
        action: 0,
        effect: SceneEffect::Robot {
            from_position: 0,
            to_position: 0,
            from_direction: 1,
            to_direction: 1,
        },
    })
}
#[test]
fn acknowledgement_replay_and_delivery_watermarks_are_exact() {
    let mut store = SceneFeedbackStore::new(EvaluationMode::Debug, n(10), n(100_000));
    store.stage(vec![wait(), wait(), wait()]).unwrap().commit();
    assert_eq!(store.last_sequence(), 3);
    assert!(matches!(
        store.read(1, n(1)),
        Err(SceneFeedbackError::InvalidCursor)
    ));
    let first = store.read(0, n(2)).unwrap();
    assert_eq!(first.next_sequence, 2);
    assert!(first.has_more);
    assert_eq!(first.events.len(), 2);
    assert_eq!(store.read(0, n(2)).unwrap(), first);
    assert_eq!(store.retained_events(), 3);
    let last = store.read(2, n(2)).unwrap();
    assert_eq!(last.next_sequence, 3);
    assert!(!last.has_more);
    assert_eq!(store.retained_events(), 1);
    assert!(matches!(
        store.read(0, n(2)),
        Err(SceneFeedbackError::InvalidCursor)
    ));
    let empty = store.read(3, n(2)).unwrap();
    assert!(empty.events.is_empty());
    assert_eq!(empty.next_sequence, 3);
    assert_eq!(store.retained_events(), 0);
    assert_eq!(store.retained_bytes(), 0);
    store.stage(vec![wait()]).unwrap().commit();
    assert_eq!(store.read(3, n(2)).unwrap().next_sequence, 4);
}
#[test]
fn abandoned_read_does_not_acknowledge_or_mark_events_delivered() {
    let mut store = SceneFeedbackStore::new(EvaluationMode::Debug, n(10), n(100_000));
    store.stage(vec![wait(), wait()]).unwrap().commit();
    {
        let prepared = store.prepare_read(0, n(2)).unwrap();
        assert_eq!(prepared.page().next_sequence, 2);
    }
    assert_eq!(store.retained_events(), 2);
    assert!(matches!(
        store.read(2, n(1)),
        Err(SceneFeedbackError::InvalidCursor)
    ));
    store.read(0, n(1)).unwrap();
    {
        let prepared = store.prepare_read(1, n(1)).unwrap();
        assert_eq!(prepared.page().next_sequence, 2);
    }
    assert_eq!(store.retained_events(), 2);
    assert!(matches!(
        store.read(2, n(1)),
        Err(SceneFeedbackError::InvalidCursor)
    ));
    store.read(1, n(1)).unwrap();
    assert_eq!(store.retained_events(), 1);
}
#[test]
fn event_and_encoded_byte_limits_are_atomic_and_released_on_acknowledgement() {
    let mut store = SceneFeedbackStore::new(EvaluationMode::Debug, n(2), n(100_000));
    let event = SceneEvent {
        sequence: 1,
        draft: wait(),
    };
    let size = serde_json::to_vec(&event).unwrap().len() as u64;
    store.stage(vec![wait()]).unwrap().commit();
    assert_eq!(store.retained_bytes(), size);
    assert!(matches!(
        store.stage(vec![wait(), wait()]),
        Err(SceneFeedbackError::ResourceLimit)
    ));
    assert_eq!(store.last_sequence(), 1);
    {
        let batch = store.stage(vec![wait()]).unwrap();
        assert_eq!(batch.encoded_bytes(), size);
    }
    assert_eq!(store.last_sequence(), 1);
    assert_eq!(store.retained_events(), 1);
    let mut store = SceneFeedbackStore::new(EvaluationMode::Debug, n(2), n(size));
    store.stage(vec![wait()]).unwrap().commit();
    assert!(matches!(
        store.stage(vec![wait()]),
        Err(SceneFeedbackError::ResourceLimit)
    ));
    store.read(0, n(1)).unwrap();
    store.read(1, n(1)).unwrap();
    store.stage(vec![wait()]).unwrap().commit();
    assert_eq!(store.last_sequence(), 2);
}
#[test]
fn official_feedback_and_oversized_pages_are_rejected() {
    let mut store = SceneFeedbackStore::new(EvaluationMode::Official, n(2), n(1000));
    assert!(matches!(
        store.stage(vec![wait()]),
        Err(SceneFeedbackError::InvalidConfiguration)
    ));
    assert!(matches!(
        store.read(0, n(1)),
        Err(SceneFeedbackError::InvalidConfiguration)
    ));
    let mut store = SceneFeedbackStore::new(EvaluationMode::Debug, n(2), n(1000));
    assert!(matches!(
        store.read(0, n(3)),
        Err(SceneFeedbackError::InvalidMaxEvents)
    ));
}
#[test]
fn wide_counters_and_visible_failures_have_exact_wire_shapes() {
    let event = SceneEvent {
        sequence: u64::MAX,
        draft: SceneEventDraft {
            tick: Some(u64::MAX),
            source_index: usize::MAX,
            ..wait()
        },
    };
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["sequence"], u64::MAX.to_string());
    assert_eq!(value["tick"], u64::MAX.to_string());
    assert_eq!(value["source_index"], usize::MAX.to_string());
    assert_eq!(value.as_object().unwrap().len(), 8);
    assert_eq!(value["actor"], "A");
    let failure = VisibleSceneFailure::Scene {
        failure: SceneFailure::InvalidOutput {
            actor: Some(1),
            value: 255,
        },
        tick: Some(8),
        frame_index: Some(2),
        pending_actions: None,
    };
    let value = visible_failure_value(&failure, false);
    assert_eq!(value.as_object().unwrap().len(), 4);
    assert_eq!(value["code"], "level.test_failed");
    assert_eq!(value["error_number"], "9027");
    assert_eq!(value["details"]["actor"], "B");
    let failure = VisibleSceneFailure::Scene {
        failure: SceneFailure::IncompleteGoal,
        tick: Some(8),
        frame_index: None,
        pending_actions: Some(1),
    };
    assert_eq!(
        visible_failure_value(&failure, true)["code"],
        "level.incomplete_output"
    );
    assert_eq!(
        visible_failure_value(&failure, false)["code"],
        "level.test_failed"
    );
    let event = SceneEvent {
        sequence: 1,
        draft: draft(SceneEventPayload::CaseEnded {
            outcome: VisibleSceneOutcome::SceneFailure,
            failure: Some(failure),
        }),
    };
    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["data"]["outcome"], "SceneFailure");
    assert!(!value.to_string().contains("true_inspection"));
}

#[test]
fn page_peak_reservation_counts_owned_observations_and_changes_no_cursor_on_failure() {
    let mut store = SceneFeedbackStore::new(EvaluationMode::Debug, n(10), n(100_000));
    store
        .stage(vec![draft(SceneEventPayload::InputAppended {
            bytes: vec![9, 8, 7],
        })])
        .unwrap()
        .commit();
    assert_eq!(store.retained_scene_units(), 4);
    assert!(matches!(
        store.prepare_read_with_scene_units(0, n(1), 3),
        Err(SceneFeedbackError::ResourceLimit)
    ));
    assert!(matches!(
        store.read(1, n(1)),
        Err(SceneFeedbackError::InvalidCursor)
    ));
    let page = store
        .prepare_read_with_scene_units(0, n(1), 4)
        .unwrap()
        .commit();
    assert_eq!(page.next_sequence, 1);
    assert_eq!(page.events.len(), 1);
    assert_eq!(store.retained_scene_units(), 4);
    let empty = store
        .prepare_read_with_scene_units(1, n(1), 0)
        .unwrap()
        .commit();
    assert!(empty.events.is_empty());
    assert_eq!(store.retained_scene_units(), 0);
}

#[test]
fn publication_encoding_is_retained_for_exact_replay() {
    let mut store = SceneFeedbackStore::new(EvaluationMode::Debug, n(10), n(100_000));
    let (batch, work) = store.stage_with_accounting(vec![
        wait(),
        draft(SceneEventPayload::InputAppended {
            bytes: vec![0, 255],
        }),
    ]);
    batch.unwrap().commit();
    assert_eq!(work, store.retained_bytes());
    let original = {
        let read = store.prepare_read(0, n(10)).unwrap();
        let texts = read
            .encoded_event_texts()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for (text, event) in texts.iter().zip(&read.page().events) {
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(text).unwrap(),
                serde_json::to_value(event).unwrap()
            );
        }
        texts
    };
    let read = store.prepare_read(0, n(10)).unwrap();
    assert_eq!(
        read.encoded_event_texts()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
        original
    );
    read.commit();
    assert_eq!(store.retained_bytes(), work);
}
