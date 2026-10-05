//! Typed visible feedback and bounded acknowledgement cursors.
use crate::{
    scene_evaluate::{VisibleSceneFailure, VisibleSceneOutcome},
    scene_protocol::SceneFailure,
    scenes::SceneKind,
    EvaluationMode,
};
use serde::{Serialize, Serializer};
use serde_json::{json, Value};
use std::{collections::VecDeque, num::NonZeroU64};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneActor {
    A,
    B,
}
impl Serialize for SceneActor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::A => "A",
            Self::B => "B",
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneInteraction {
    Input,
    Output,
    BufferLeft,
    BufferRight,
    Worktable(usize),
    Unavailable,
}
impl SceneInteraction {
    pub fn name(self) -> &'static str {
        match self {
            Self::Input => "Input",
            Self::Output => "Output",
            Self::BufferLeft => "BufferLeft",
            Self::BufferRight => "BufferRight",
            Self::Worktable(_) => "Worktable",
            Self::Unavailable => "Unavailable",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneEffect {
    Elevator {
        from_floor: u8,
        to_floor: u8,
        boarded: u64,
        delivered: u64,
    },
    Robot {
        from_position: u8,
        to_position: u8,
        from_direction: u8,
        to_direction: u8,
    },
    MechanicalArm {
        interaction: SceneInteraction,
        from_orientation: u8,
        to_orientation: u8,
        before_state: u8,
        after_state: u8,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneRoundChange {
    TableReady { worktable_index: usize, state: u8 },
    BufferReady { right: bool, state: u8 },
    DoorOpened { door_id: u8 },
    PassengerIntroduced { from: u8, to: u8 },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneEventPayload {
    CaseStarted {
        observation: Option<Vec<u8>>,
    },
    ActionApplied {
        action: u8,
        effect: SceneEffect,
    },
    RoundCompleted {
        rounds: u64,
        observation: Vec<u8>,
        transitions: Vec<SceneRoundChange>,
    },
    InputAppended {
        bytes: Vec<u8>,
    },
    RobotProduced {
        state: u8,
        output_index: u64,
    },
    CaseEnded {
        outcome: VisibleSceneOutcome,
        failure: Option<VisibleSceneFailure>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneEventDraft {
    pub source_index: usize,
    pub scene_type: SceneKind,
    pub tick: Option<u64>,
    pub frame_index: Option<u64>,
    pub actor: Option<SceneActor>,
    pub payload: SceneEventPayload,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneEvent {
    pub sequence: u64,
    pub draft: SceneEventDraft,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneFeedbackPage {
    pub events: Vec<SceneEvent>,
    pub next_sequence: u64,
    pub has_more: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneFeedbackError {
    InvalidConfiguration,
    InvalidCursor,
    InvalidMaxEvents,
    ResourceLimit,
    NumericOverflow,
}

pub fn visible_outcome_name(outcome: VisibleSceneOutcome) -> &'static str {
    match outcome {
        VisibleSceneOutcome::Passed => "Passed",
        VisibleSceneOutcome::WrongOutput => "WrongOutput",
        VisibleSceneOutcome::IncompleteOutput => "IncompleteOutput",
        VisibleSceneOutcome::SceneFailure => "SceneFailure",
        VisibleSceneOutcome::RuntimeError => "RuntimeError",
        VisibleSceneOutcome::NotCompleted => "NotCompleted",
    }
}
/// Shared fixed-version payload serialization; no world or author records enter it.
pub fn visible_failure_value(failure: &VisibleSceneFailure, static_scene: bool) -> Value {
    let mut details = serde_json::Map::new();
    let (code, reason) = match failure {
        VisibleSceneFailure::RuntimeError { codes } => {
            details.insert("runtime_errors".into(),Value::Array(codes.iter().map(|code|json!({"code":code,"error_number":codegrid_model::error_number("vm",code)})).collect()));
            ("level.runtime_error", "RuntimeError")
        }
        VisibleSceneFailure::Scene {
            failure,
            tick,
            frame_index,
            pending_actions,
        } => {
            if let Some(tick) = tick {
                details.insert("tick".into(), json!(tick.to_string()));
            }
            if let Some(frame) = frame_index {
                details.insert("frame_index".into(), json!(frame.to_string()));
            }
            if let Some(count) = pending_actions {
                details.insert("pending_actions".into(), json!(count));
            }
            match failure {
                SceneFailure::InvalidOutput { actor, value } => {
                    if let Some(actor) = actor {
                        details.insert("actor".into(), json!(if *actor == 0 { "A" } else { "B" }));
                    }
                    details.insert("value".into(), json!(value));
                    ("level.test_failed", "InvalidOutput")
                }
                SceneFailure::IllegalOperation {
                    actor, interaction, ..
                } => {
                    details.insert("interaction".into(), json!(interaction.name()));
                    if let SceneInteraction::Worktable(index) = interaction {
                        details.insert("worktable_index".into(), json!(index));
                    }
                    details.insert("actor".into(), json!(if *actor == 0 { "A" } else { "B" }));
                    ("level.test_failed", "IllegalOperation")
                }
                SceneFailure::WrongOutput {
                    actor,
                    index,
                    expected,
                    actual,
                } => {
                    if let Some(actor) = actor {
                        details.insert("actor".into(), json!(if *actor == 0 { "A" } else { "B" }));
                    }
                    details.insert("output_index".into(), json!(index.to_string()));
                    if let Some(expected) = expected {
                        details.insert("expected".into(), json!(expected));
                    }
                    details.insert("actual".into(), json!(actual));
                    ("level.wrong_output", "WrongOutput")
                }
                SceneFailure::IncompleteGoal if static_scene => {
                    ("level.incomplete_output", "IncompleteOutput")
                }
                SceneFailure::IncompleteGoal => ("level.test_failed", "IncompleteGoal"),
            }
        }
    };
    json!({"code":code,"error_number":codegrid_model::error_number("level",code),"reason":reason,"details":details})
}
impl SceneEffect {
    fn value(&self) -> Value {
        match self {
            Self::Elevator {
                from_floor,
                to_floor,
                boarded,
                delivered,
            } => {
                json!({"scene_type":"Elevator","from_floor":from_floor,"to_floor":to_floor,"boarded":boarded.to_string(),"delivered":delivered.to_string()})
            }
            Self::Robot {
                from_position,
                to_position,
                from_direction,
                to_direction,
            } => {
                json!({"scene_type":"Robot","from_position":from_position,"to_position":to_position,"from_direction":from_direction,"to_direction":to_direction})
            }
            Self::MechanicalArm {
                interaction,
                from_orientation,
                to_orientation,
                before_state,
                after_state,
            } => {
                let mut value = json!({"scene_type":"MechanicalArm","interaction":interaction.name(),"from_orientation":from_orientation,"to_orientation":to_orientation,"before_state":before_state,"after_state":after_state});
                if let SceneInteraction::Worktable(index) = interaction {
                    value["worktable_index"] = json!(index);
                }
                value
            }
        }
    }
}
impl SceneRoundChange {
    fn value(&self) -> Value {
        match self {
            Self::TableReady {
                worktable_index,
                state,
            } => json!({"kind":"TableReady","worktable_index":worktable_index,"state":state}),
            Self::BufferReady { right, state } => {
                json!({"kind":"BufferReady","buffer":if *right {"Right"}else{"Left"},"state":state})
            }
            Self::DoorOpened { door_id } => json!({"kind":"DoorOpened","door_id":door_id}),
            Self::PassengerIntroduced { from, to } => {
                json!({"kind":"PassengerIntroduced","from":from,"to":to})
            }
        }
    }
}
impl SceneEventPayload {
    pub fn retained_scene_units(&self) -> u64 {
        1 + match self {
            Self::CaseStarted { observation } => observation.as_ref().map_or(0, |v| v.len() as u64),
            Self::RoundCompleted {
                observation,
                transitions,
                ..
            } => observation.len() as u64 + transitions.len() as u64,
            Self::InputAppended { bytes } => bytes.len() as u64,
            _ => 0,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::CaseStarted { .. } => "CaseStarted",
            Self::ActionApplied { .. } => "ActionApplied",
            Self::RoundCompleted { .. } => "RoundCompleted",
            Self::InputAppended { .. } => "InputAppended",
            Self::RobotProduced { .. } => "RobotProduced",
            Self::CaseEnded { .. } => "CaseEnded",
        }
    }
    fn value(&self, static_scene: bool) -> Value {
        match self {
            Self::CaseStarted { observation } => json!({"observation":observation}),
            Self::ActionApplied { action, effect } => {
                json!({"action":action,"effect":effect.value()})
            }
            Self::RoundCompleted {
                rounds,
                observation,
                transitions,
            } => {
                json!({"rounds":rounds.to_string(),"observation":observation,"transitions":transitions.iter().map(SceneRoundChange::value).collect::<Vec<_>>()})
            }
            Self::InputAppended { bytes } => json!({"bytes":bytes}),
            Self::RobotProduced {
                state,
                output_index,
            } => json!({"state":state,"output_index":output_index.to_string()}),
            Self::CaseEnded { outcome, failure } => {
                json!({"outcome":visible_outcome_name(*outcome),"failure":failure.as_ref().map(|f|visible_failure_value(f,static_scene))})
            }
        }
    }
}
impl Serialize for SceneEvent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let d = &self.draft;
        let static_scene = matches!(
            d.scene_type,
            SceneKind::ExactIO | SceneKind::Baudot | SceneKind::QualityControl
        );
        json!({"sequence":self.sequence.to_string(),"source_index":d.source_index.to_string(),"scene_type":d.scene_type.id(),
            "tick":d.tick.map(|v|v.to_string()),"frame_index":d.frame_index.map(|v|v.to_string()),"actor":d.actor,"kind":d.payload.kind(),"data":d.payload.value(static_scene)}).serialize(serializer)
    }
}

struct RetainedEvent {
    event: SceneEvent,
    encoded: String,
    bytes: u64,
    units: u64,
}
pub struct SceneFeedbackStore {
    mode: EvaluationMode,
    events: VecDeque<RetainedEvent>,
    acknowledged: u64,
    delivered: u64,
    published: u64,
    bytes: u64,
    units: u64,
    other_bytes: u64,
    end_reservation: u64,
    max_events: u64,
    max_bytes: u64,
}
pub struct SceneFeedbackBatch<'a> {
    store: &'a mut SceneFeedbackStore,
    events: Vec<RetainedEvent>,
    bytes: u64,
    last_sequence: u64,
    units: u64,
    ends_case: bool,
}
impl SceneFeedbackBatch<'_> {
    pub fn retained_scene_units(&self) -> u64 {
        self.units
    }
    pub fn current_scene_units(&self) -> u64 {
        self.store.units
    }
    pub fn encoded_bytes(&self) -> u64 {
        self.bytes
    }
    /// Exclusive staging prevents intervening/cross-store publication.
    pub fn commit(self) {
        self.store.bytes += self.bytes;
        self.store.units += self.units;
        if self.ends_case {
            self.store.end_reservation = 0;
        }
        self.store.published = self.last_sequence;
        self.store.events.extend(self.events);
    }
}
impl SceneFeedbackStore {
    pub fn new(mode: EvaluationMode, max_events: NonZeroU64, max_bytes: NonZeroU64) -> Self {
        Self {
            mode,
            events: VecDeque::new(),
            acknowledged: 0,
            delivered: 0,
            published: 0,
            bytes: 0,
            units: 0,
            other_bytes: 0,
            end_reservation: 0,
            max_events: max_events.get(),
            max_bytes: max_bytes.get(),
        }
    }
    pub fn retained_scene_units(&self) -> u64 {
        self.units
    }
    pub fn set_other_feedback_bytes_for_end(
        &mut self,
        bytes: u64,
    ) -> Result<(), SceneFeedbackError> {
        if bytes
            .checked_add(self.bytes)
            .is_none_or(|n| n > self.max_bytes)
        {
            return Err(SceneFeedbackError::ResourceLimit);
        }
        self.other_bytes = bytes;
        Ok(())
    }
    pub fn set_other_feedback_bytes(&mut self, bytes: u64) -> Result<(), SceneFeedbackError> {
        if bytes
            .checked_add(self.bytes)
            .and_then(|n| n.checked_add(self.end_reservation))
            .is_none_or(|n| n > self.max_bytes)
        {
            return Err(SceneFeedbackError::ResourceLimit);
        }
        self.other_bytes = bytes;
        Ok(())
    }
    pub fn reserve_case_end(&mut self, bound: u64) -> Result<(), SceneFeedbackError> {
        if self.end_reservation != 0 {
            return Err(SceneFeedbackError::InvalidConfiguration);
        }
        if self.events.len() as u64 >= self.max_events
            || self
                .bytes
                .checked_add(self.other_bytes)
                .and_then(|n| n.checked_add(bound))
                .is_none_or(|n| n > self.max_bytes)
        {
            return Err(SceneFeedbackError::ResourceLimit);
        }
        self.end_reservation = bound;
        Ok(())
    }
    pub fn abandon_case_end(&mut self) {
        self.end_reservation = 0;
    }
    pub fn retained_bytes(&self) -> u64 {
        self.bytes
    }
    pub fn retained_events(&self) -> u64 {
        self.events.len() as u64
    }
    pub fn last_sequence(&self) -> u64 {
        self.published
    }
    /// Reserve a complete batch before the caller publishes its world/input draft.
    pub fn stage(
        &mut self,
        drafts: Vec<SceneEventDraft>,
    ) -> Result<SceneFeedbackBatch<'_>, SceneFeedbackError> {
        self.stage_with_accounting(drafts).0
    }
    pub fn stage_with_accounting(
        &mut self,
        drafts: Vec<SceneEventDraft>,
    ) -> (Result<SceneFeedbackBatch<'_>, SceneFeedbackError>, u64) {
        let mut work = 0;
        let result = self.stage_counted(drafts, &mut work);
        (result, work)
    }
    fn stage_counted(
        &mut self,
        drafts: Vec<SceneEventDraft>,
        work: &mut u64,
    ) -> Result<SceneFeedbackBatch<'_>, SceneFeedbackError> {
        if self.mode != EvaluationMode::Debug {
            return Err(SceneFeedbackError::InvalidConfiguration);
        }
        let ends_case = drafts
            .iter()
            .any(|d| matches!(d.payload, SceneEventPayload::CaseEnded { .. }));
        let end_records = u64::from(self.end_reservation != 0 && !ends_case);
        let count = (self.events.len() as u64)
            .checked_add(drafts.len() as u64)
            .and_then(|n| n.checked_add(end_records))
            .ok_or(SceneFeedbackError::NumericOverflow)?;
        if count > self.max_events {
            return Err(SceneFeedbackError::ResourceLimit);
        }
        let mut sequence = self.published;
        let mut bytes = 0u64;
        let mut units = 0u64;
        let mut events = Vec::new();
        events
            .try_reserve(drafts.len())
            .map_err(|_| SceneFeedbackError::ResourceLimit)?;
        for draft in drafts {
            sequence = sequence
                .checked_add(1)
                .ok_or(SceneFeedbackError::NumericOverflow)?;
            let event = SceneEvent { sequence, draft };
            let encoded =
                serde_json::to_string(&event).map_err(|_| SceneFeedbackError::NumericOverflow)?;
            let size = encoded.len() as u64;
            *work = work
                .checked_add(size)
                .ok_or(SceneFeedbackError::NumericOverflow)?;
            bytes = bytes
                .checked_add(size)
                .ok_or(SceneFeedbackError::NumericOverflow)?;
            if self
                .bytes
                .checked_add(bytes)
                .and_then(|n| n.checked_add(self.other_bytes))
                .and_then(|n| n.checked_add(if ends_case { 0 } else { self.end_reservation }))
                .is_none_or(|n| n > self.max_bytes)
            {
                return Err(SceneFeedbackError::ResourceLimit);
            }
            let retained = event.draft.payload.retained_scene_units();
            units = units
                .checked_add(retained)
                .ok_or(SceneFeedbackError::NumericOverflow)?;
            events.push(RetainedEvent {
                event,
                encoded,
                bytes: size,
                units: retained,
            });
        }
        self.events
            .try_reserve(events.len())
            .map_err(|_| SceneFeedbackError::ResourceLimit)?;
        Ok(SceneFeedbackBatch {
            store: self,
            events,
            bytes,
            last_sequence: sequence,
            units,
            ends_case,
        })
    }
    pub fn read(
        &mut self,
        after_sequence: u64,
        max_events: NonZeroU64,
    ) -> Result<SceneFeedbackPage, SceneFeedbackError> {
        Ok(self.prepare_read(after_sequence, max_events)?.commit())
    }
    /// A host checks the complete encoded response before committing delivery.
    pub fn prepare_read(
        &mut self,
        after_sequence: u64,
        max_events: NonZeroU64,
    ) -> Result<SceneFeedbackRead<'_>, SceneFeedbackError> {
        self.prepare_read_with_scene_units(after_sequence, max_events, u64::MAX)
    }
    /// Reserve page copies independently from retained events before cloning.
    pub fn prepare_read_with_scene_units(
        &mut self,
        after_sequence: u64,
        max_events: NonZeroU64,
        available_scene_units: u64,
    ) -> Result<SceneFeedbackRead<'_>, SceneFeedbackError> {
        if self.mode != EvaluationMode::Debug {
            return Err(SceneFeedbackError::InvalidConfiguration);
        }
        if max_events.get() > self.max_events {
            return Err(SceneFeedbackError::InvalidMaxEvents);
        }
        if after_sequence < self.acknowledged || after_sequence > self.delivered {
            return Err(SceneFeedbackError::InvalidCursor);
        }
        let available = self
            .events
            .iter()
            .filter(|e| e.event.sequence > after_sequence);
        let take = max_events.get().min(self.events.len() as u64) as usize;
        let mut events = Vec::new();
        events
            .try_reserve(take)
            .map_err(|_| SceneFeedbackError::ResourceLimit)?;
        let mut has_more = false;
        let mut page_units = 0u64;
        for event in available {
            if events.len() == take {
                has_more = true;
                break;
            }
            page_units = page_units
                .checked_add(event.units)
                .ok_or(SceneFeedbackError::NumericOverflow)?;
            if page_units > available_scene_units {
                return Err(SceneFeedbackError::ResourceLimit);
            }
            events.push(event.event.clone());
        }
        let next_sequence = events.last().map_or(after_sequence, |e| e.sequence);
        Ok(SceneFeedbackRead {
            store: self,
            after_sequence,
            page: SceneFeedbackPage {
                events,
                next_sequence,
                has_more,
            },
        })
    }
}
/// No acknowledgement or delivery watermark changes if this preparation is dropped.
pub struct SceneFeedbackRead<'a> {
    store: &'a mut SceneFeedbackStore,
    after_sequence: u64,
    page: SceneFeedbackPage,
}
impl SceneFeedbackRead<'_> {
    pub fn page(&self) -> &SceneFeedbackPage {
        &self.page
    }
    /// Immutable publication encoding; replay copies bytes without re-encoding.
    pub fn encoded_event_texts(&self) -> impl Iterator<Item = &str> {
        self.store
            .events
            .iter()
            .skip_while(|event| event.event.sequence <= self.after_sequence)
            .take(self.page.events.len())
            .map(|event| event.encoded.as_str())
    }
    pub fn commit(self) -> SceneFeedbackPage {
        while self
            .store
            .events
            .front()
            .is_some_and(|e| e.event.sequence <= self.after_sequence)
        {
            let event = self.store.events.pop_front().unwrap();
            self.store.bytes -= event.bytes;
            self.store.units -= event.units;
        }
        self.store.acknowledged = self.after_sequence;
        self.store.delivered = self.store.delivered.max(self.page.next_sequence);
        self.page
    }
}
