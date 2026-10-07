//! Actor framing and terminal ordering shared by all scene sessions.
use crate::scene_feedback::{SceneActor, SceneEventPayload, SceneInteraction};
use crate::scene_runtime::{
    MechanicalArmRuntime, RobotRuntime, SceneCommand, SceneRuntime, SceneRuntimeMode, StaticRuntime,
};
use crate::scene_world::ActorSnapshot;
use crate::scenes::SceneKind;
use crate::{SceneCaseData, SceneConfig, ValidatedSceneLevel};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneFailure {
    InvalidOutput {
        actor: Option<u8>,
        value: u8,
    },
    IllegalOperation {
        actor: u8,
        operation: IllegalOperationReason,
        interaction: SceneInteraction,
    },
    WrongOutput {
        actor: Option<u8>,
        index: usize,
        expected: Option<u8>,
        actual: u8,
    },
    IncompleteGoal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IllegalOperationReason {
    OccupiedHand,
    OccupiedTarget,
    InvalidDropTarget,
    InvalidTableInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneOutcome {
    Running,
    Passed,
    Failed(SceneFailure),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneCounterError {
    Overflow,
}
/// A scene is constructed only from a validated author definition.
///
/// This type owns protocol transitions, not VM execution or host resource policy.
/// Callers must stage a clone and publish it together with its input append after
/// checking limits. Do not expose this state for hidden cases at host boundaries.
#[derive(Clone)]
pub struct SceneMachine {
    runtime: Box<dyn SceneRuntime>,
    actors: usize,
    pending: Vec<u8>,
    outcome: SceneOutcome,
    frames: u64,
    initial_input: Vec<u8>,
    work: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneTransition {
    pub observation: Vec<u8>,
    pub completed_frame: bool,
    pub outcome: SceneOutcome,
    pub work: u64,
    pub events: Vec<SceneProtocolEvent>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneProtocolEvent {
    pub actor: Option<SceneActor>,
    pub payload: SceneEventPayload,
}
impl SceneMachine {
    pub fn new(level: &ValidatedSceneLevel, source_index: usize) -> Option<Self> {
        let case = level.tests().get(source_index)?;
        let (mut runtime, actors): (Box<dyn SceneRuntime>, usize) =
            match (&case.data, level.config()) {
                (SceneCaseData::Static { input, expected }, _) => (
                    Box::new(StaticRuntime::new(level.kind(), input, expected))
                        as Box<dyn SceneRuntime>,
                    1,
                ),
                (SceneCaseData::Robot, SceneConfig::Robot { actors, map }) => (
                    Box::new(RobotRuntime::new(&map.starts, &map.cells)),
                    usize::from(*actors),
                ),
                (
                    SceneCaseData::MechanicalArm { input, expected },
                    SceneConfig::MechanicalArm { actors, tables },
                ) => (
                    Box::new(MechanicalArmRuntime::new(*actors, *tables, input, expected)),
                    usize::from(*actors),
                ),
                _ => return None,
            };
        let initial_complete = runtime.initial_complete();
        let outcome = if initial_complete {
            SceneOutcome::Passed
        } else {
            SceneOutcome::Running
        };
        let initial_input = if outcome != SceneOutcome::Running {
            Vec::new()
        } else {
            runtime.take_initial_input()
        };
        Some(Self {
            runtime,
            actors,
            pending: Vec::new(),
            outcome,
            frames: 0,
            initial_input,
            work: 0,
        })
    }
    pub fn initial_input(&self) -> &[u8] {
        &self.initial_input
    }
    /// Transfer initial input ownership into the VM queue exactly once.
    pub fn take_initial_input(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.initial_input)
    }
    pub fn initialization_work(&self) -> u64 {
        self.retained_units()
            + self
                .runtime
                .initialization_work(self.actors, self.initial_input.len())
    }
    pub fn retained_units(&self) -> u64 {
        self.pending.len() as u64 + self.initial_input.len() as u64 + self.runtime.retained_units()
    }
    /// Reserve this bound before a non-resumable transition, then charge actual work.
    pub fn transition_work_bound(&self) -> Option<u64> {
        self.retained_units().checked_mul(8)?.checked_add(64)
    }
    pub fn will_complete_frame(&self) -> bool {
        self.runtime.mode() == SceneRuntimeMode::ActorFrame && self.pending.len() + 1 == self.actors
    }
    pub fn summary(&self) -> std::collections::BTreeMap<&'static str, u64> {
        let mut result = std::collections::BTreeMap::new();
        if self.runtime.mode() == SceneRuntimeMode::ActorFrame {
            result.insert("frames", self.frames);
            result.insert("rounds", self.rounds());
        }
        result.extend(self.runtime.summary());
        result
    }
    pub fn expected_output(&self) -> Option<&[u8]> {
        self.runtime.expected_output()
    }
    pub fn outcome(&self) -> &SceneOutcome {
        &self.outcome
    }
    pub fn frames(&self) -> u64 {
        self.frames
    }
    pub fn rounds(&self) -> u64 {
        self.runtime.rounds()
    }
    pub fn pending_actions(&self) -> &[u8] {
        &self.pending
    }
    pub fn actual_output(&self) -> Option<&[u8]> {
        self.runtime.actual_output()
    }
    /// Consume committed outer output once. Never dispatch an incomplete frame.
    pub fn consume(&mut self, value: u8) -> Result<SceneTransition, SceneCounterError> {
        self.consume_inner(value, false)
    }
    pub fn consume_debug(&mut self, value: u8) -> Result<SceneTransition, SceneCounterError> {
        self.consume_inner(value, true)
    }
    fn debug_snapshot(&mut self, actor: usize) -> ActorSnapshot {
        self.runtime
            .debug_snapshot(actor)
            .expect("actor frame runtime provides actor snapshots")
    }
    fn consume_inner(
        &mut self,
        value: u8,
        feedback: bool,
    ) -> Result<SceneTransition, SceneCounterError> {
        let mut events = Vec::new();
        self.work = 0;
        self.runtime.reset_transition_work();
        if self.outcome != SceneOutcome::Running {
            return self.transition(Vec::new(), false, events);
        }
        if self.runtime.mode() == SceneRuntimeMode::StaticOutput {
            let transition = self.runtime.transition(SceneCommand::StaticOutput(value))?;
            self.outcome = transition
                .outcome
                .expect("static comparison always returns its outcome");
            return self.transition(Vec::new(), false, events);
        }
        self.work += 1; // Frame byte retained.
        self.pending.push(value);
        if self.pending.len() != self.actors {
            return self.transition(Vec::new(), false, events);
        }
        self.frames = self
            .frames
            .checked_add(1)
            .ok_or(SceneCounterError::Overflow)?;
        let actions = std::mem::take(&mut self.pending);
        let mut observation = Vec::new();
        for (actor, action) in actions.into_iter().enumerate() {
            self.work += 1; // Actor byte decoded.
            let before = feedback.then(|| self.debug_snapshot(actor));
            let produced = self.actual_output().map_or(0, |a| a.len());
            let action_transition = self
                .runtime
                .transition(SceneCommand::ActorOutput { actor, action })?;
            observation.extend(action_transition.observation);
            if let Some(outcome) = action_transition.outcome {
                self.outcome = outcome;
            }
            if let Some(before) = before {
                if !matches!(
                    &self.outcome,
                    SceneOutcome::Failed(
                        SceneFailure::InvalidOutput { .. } | SceneFailure::IllegalOperation { .. }
                    )
                ) {
                    let effect = before.effect(self.debug_snapshot(actor));
                    let actor = Some(if actor == 0 {
                        SceneActor::A
                    } else {
                        SceneActor::B
                    });
                    events.push(SceneProtocolEvent {
                        actor,
                        payload: SceneEventPayload::ActionApplied { action, effect },
                    });
                    if self.runtime.kind() == SceneKind::MechanicalArm
                        && self
                            .actual_output()
                            .is_some_and(|actual| actual.len() > produced)
                    {
                        self.runtime.record_action_feedback_work();
                        let state = self.actual_output().unwrap()[produced];
                        events.push(SceneProtocolEvent {
                            actor,
                            payload: SceneEventPayload::RobotProduced {
                                state,
                                output_index: produced as u64,
                            },
                        });
                    }
                }
            }
            if self.outcome != SceneOutcome::Running {
                return self.transition(Vec::new(), true, events);
            }
        }
        let previous_rounds = self.rounds();
        let changes = if feedback {
            self.runtime.round_changes()?
        } else {
            Vec::new()
        };
        observation = self.runtime.end_round(observation)?;
        if feedback && self.rounds() > previous_rounds {
            let snapshot = if self.runtime.kind() == SceneKind::MechanicalArm && self.actors == 1 {
                Vec::new()
            } else {
                observation.clone()
            };
            self.work += snapshot.len() as u64;
            events.push(SceneProtocolEvent {
                actor: None,
                payload: SceneEventPayload::RoundCompleted {
                    rounds: self.rounds(),
                    observation: snapshot,
                    transitions: changes,
                },
            });
        }
        self.transition(observation, true, events)
    }
    pub fn halt(&mut self) {
        if self.outcome != SceneOutcome::Running {
            return;
        }
        self.outcome = if self.runtime.mode() == SceneRuntimeMode::StaticOutput
            && self.runtime.expected_output().is_some_and(<[u8]>::is_empty)
            && self.runtime.actual_output().is_some_and(<[u8]>::is_empty)
        {
            SceneOutcome::Passed
        } else {
            SceneOutcome::Failed(SceneFailure::IncompleteGoal)
        };
    }
    fn transition(
        &self,
        observation: Vec<u8>,
        completed_frame: bool,
        events: Vec<SceneProtocolEvent>,
    ) -> Result<SceneTransition, SceneCounterError> {
        let work = self
            .work
            .checked_add(observation.len() as u64)
            .ok_or(SceneCounterError::Overflow)?
            .checked_add(
                self.runtime
                    .transition_work(self.actors, observation.is_empty()),
            )
            .ok_or(SceneCounterError::Overflow)?;
        Ok(SceneTransition {
            completed_frame,
            events,
            outcome: self.outcome.clone(),
            work,
            observation,
        })
    }
}

#[cfg(test)]
mod runtime_seam_tests {
    use super::*;
    use serde_json::Value;

    fn example(name: &str) -> Value {
        let json = match name {
            "exact" => include_str!("../../../examples/scene-level-v2/exactio.json"),
            "robot" => include_str!("../../../examples/scene-level-v2/robot.json"),
            "arm" => include_str!("../../../examples/scene-level-v2/mechanical-arm.json"),
            _ => panic!("unknown scene fixture"),
        };
        serde_json::from_str(json).unwrap()
    }

    fn machine(name: &str) -> SceneMachine {
        let level = crate::load_scene_level_json(
            example(name).to_string().as_bytes(),
            crate::LoadLimits {
                max_level_bytes: 1_000_000,
                max_tests: 20,
                max_total_test_bytes: 1_000_000,
            },
        )
        .unwrap();
        SceneMachine::new(&level, 0).unwrap()
    }

    #[test]
    fn every_builtin_scene_uses_the_object_safe_transition_seam() {
        let cases = [
            (
                "exact",
                SceneKind::ExactIO,
                SceneRuntimeMode::StaticOutput,
                9,
            ),
            ("robot", SceneKind::Robot, SceneRuntimeMode::ActorFrame, 1),
            (
                "arm",
                SceneKind::MechanicalArm,
                SceneRuntimeMode::ActorFrame,
                1,
            ),
        ];

        for (fixture, kind, mode, output) in cases {
            let mut machine = machine(fixture);
            assert_eq!(machine.runtime.kind(), kind, "{fixture} runtime kind");
            assert_eq!(machine.runtime.mode(), mode, "{fixture} transition mode");
            let transition = machine.consume(output).unwrap();
            assert!(transition.work > 0, "{fixture} transition is billed");
            assert_eq!(machine.outcome(), &transition.outcome);
        }
    }

    #[test]
    fn cloning_a_staged_scene_runtime_does_not_publish_its_transition() {
        let committed = machine("robot");
        let mut candidate = committed.clone();
        assert_eq!(candidate.consume(1).unwrap().outcome, SceneOutcome::Passed);
        assert_eq!(committed.outcome(), &SceneOutcome::Running);
        assert_eq!(committed.rounds(), 0);
        assert_eq!(candidate.rounds(), 0);
    }
}
