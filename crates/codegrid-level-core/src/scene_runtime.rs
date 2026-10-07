//! Private object-safe seam for built-in scene transitions.
//!
//! This is an in-process implementation detail. It is not a package format,
//! guest ABI, or promise that custom scenes can currently be loaded.
use crate::scene_feedback::SceneRoundChange;
use crate::scene_protocol::{
    IllegalOperationReason, SceneCounterError, SceneFailure, SceneOutcome,
};
use crate::scene_world::{ActorSnapshot, ArmError, ArmWorld, RobotWorld, TransitionError};
use crate::scenes::SceneKind;
use crate::{InputRobot, Worktable};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SceneRuntimeMode {
    StaticOutput,
    ActorFrame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SceneCommand {
    StaticOutput(u8),
    ActorOutput { actor: usize, action: u8 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RuntimeTransition {
    /// Static comparisons always return an outcome. Dynamic scenes return one
    /// only when an action fails or completes the scene goal.
    pub outcome: Option<SceneOutcome>,
    pub observation: Vec<u8>,
}

pub(super) trait SceneRuntimeClone {
    fn clone_box(&self) -> Box<dyn SceneRuntime>;
}

impl<T> SceneRuntimeClone for T
where
    T: 'static + SceneRuntime + Clone,
{
    fn clone_box(&self) -> Box<dyn SceneRuntime> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn SceneRuntime> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Common transition and observation surface for every currently built-in
/// scene. The evaluator owns framing and publication; implementations own only
/// scene-specific state transitions and deterministic data queries.
pub(super) trait SceneRuntime: SceneRuntimeClone {
    fn kind(&self) -> SceneKind;
    fn mode(&self) -> SceneRuntimeMode;
    fn reset_transition_work(&mut self);
    fn transition(&mut self, command: SceneCommand)
        -> Result<RuntimeTransition, SceneCounterError>;
    fn record_action_feedback_work(&mut self) {}
    fn initial_complete(&mut self) -> bool {
        false
    }
    fn take_initial_input(&mut self) -> Vec<u8>;
    fn initialization_work(&self, actors: usize, initial_input_len: usize) -> u64;
    fn retained_units(&self) -> u64;
    fn transition_work(&self, actors: usize, observation_is_empty: bool) -> u64;
    fn summary(&self) -> BTreeMap<&'static str, u64>;
    fn expected_output(&self) -> Option<&[u8]>;
    fn actual_output(&self) -> Option<&[u8]>;
    fn rounds(&self) -> u64;
    fn debug_snapshot(&mut self, actor: usize) -> Option<ActorSnapshot>;
    fn round_changes(&mut self) -> Result<Vec<SceneRoundChange>, SceneCounterError>;
    fn end_round(&mut self, observation: Vec<u8>) -> Result<Vec<u8>, SceneCounterError>;
}

#[derive(Clone)]
pub(super) struct StaticRuntime {
    kind: SceneKind,
    input: Vec<u8>,
    expected: Vec<u8>,
    actual: Vec<u8>,
    work: u64,
}

impl StaticRuntime {
    pub fn new(kind: SceneKind, input: &[u8], expected: &[u8]) -> Self {
        Self {
            kind,
            input: input.to_vec(),
            expected: expected.to_vec(),
            actual: Vec::new(),
            work: 0,
        }
    }
}

impl SceneRuntime for StaticRuntime {
    fn kind(&self) -> SceneKind {
        self.kind
    }

    fn mode(&self) -> SceneRuntimeMode {
        SceneRuntimeMode::StaticOutput
    }

    fn reset_transition_work(&mut self) {
        self.work = 0;
    }

    fn transition(
        &mut self,
        command: SceneCommand,
    ) -> Result<RuntimeTransition, SceneCounterError> {
        let SceneCommand::StaticOutput(value) = command else {
            unreachable!("static runtime only receives static outputs")
        };
        let outcome = {
            self.work += 3; // Decode, inspect expected byte, update actual sequence.
            let index = self.actual.len();
            let wanted = self.expected.get(index).copied();
            self.actual.push(value);
            if wanted != Some(value) {
                SceneOutcome::Failed(SceneFailure::WrongOutput {
                    actor: None,
                    index,
                    expected: wanted,
                    actual: value,
                })
            } else if self.actual.len() == self.expected.len() {
                SceneOutcome::Passed
            } else {
                SceneOutcome::Running
            }
        };
        Ok(RuntimeTransition {
            outcome: Some(outcome),
            observation: Vec::new(),
        })
    }

    fn take_initial_input(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.input)
    }

    fn initialization_work(&self, _actors: usize, _initial_input_len: usize) -> u64 {
        0
    }

    fn retained_units(&self) -> u64 {
        (self.expected.len() + self.actual.len()) as u64
    }

    fn transition_work(&self, _actors: usize, _observation_is_empty: bool) -> u64 {
        self.work
    }

    fn summary(&self) -> BTreeMap<&'static str, u64> {
        BTreeMap::new()
    }

    fn expected_output(&self) -> Option<&[u8]> {
        Some(&self.expected)
    }

    fn actual_output(&self) -> Option<&[u8]> {
        Some(&self.actual)
    }

    fn rounds(&self) -> u64 {
        0
    }

    fn debug_snapshot(&mut self, _actor: usize) -> Option<ActorSnapshot> {
        None
    }

    fn round_changes(&mut self) -> Result<Vec<SceneRoundChange>, SceneCounterError> {
        Ok(Vec::new())
    }

    fn end_round(&mut self, _observation: Vec<u8>) -> Result<Vec<u8>, SceneCounterError> {
        unreachable!("static runtime has no actor rounds")
    }
}

#[derive(Clone)]
pub(super) struct RobotRuntime {
    world: RobotWorld,
}

impl RobotRuntime {
    pub fn new(starts: &[crate::RobotStart], cells: &[crate::MapCell]) -> Self {
        Self {
            world: RobotWorld::new(starts, cells),
        }
    }
}

impl SceneRuntime for RobotRuntime {
    fn kind(&self) -> SceneKind {
        SceneKind::Robot
    }

    fn mode(&self) -> SceneRuntimeMode {
        SceneRuntimeMode::ActorFrame
    }

    fn reset_transition_work(&mut self) {
        self.world.work = 0;
    }

    fn transition(
        &mut self,
        command: SceneCommand,
    ) -> Result<RuntimeTransition, SceneCounterError> {
        let SceneCommand::ActorOutput { actor, action } = command else {
            unreachable!("robot runtime only receives actor outputs")
        };
        let result = match self.world.act(actor, action) {
            Ok(()) => RuntimeTransition {
                outcome: self.world.complete().then_some(SceneOutcome::Passed),
                observation: Vec::new(),
            },
            Err(TransitionError::CounterOverflow) => return Err(SceneCounterError::Overflow),
            Err(TransitionError::InvalidOutput) => RuntimeTransition {
                outcome: Some(SceneOutcome::Failed(SceneFailure::InvalidOutput {
                    actor: Some(actor as u8),
                    value: action,
                })),
                observation: Vec::new(),
            },
        };
        Ok(result)
    }

    fn initial_complete(&mut self) -> bool {
        self.world.complete()
    }

    fn take_initial_input(&mut self) -> Vec<u8> {
        self.world.observation()
    }

    fn initialization_work(&self, actors: usize, initial_input_len: usize) -> u64 {
        self.world.work
            + if initial_input_len == 0 {
                0
            } else {
                actors as u64 * 2
            }
    }

    fn retained_units(&self) -> u64 {
        self.world.retained_units()
    }

    fn transition_work(&self, actors: usize, observation_is_empty: bool) -> u64 {
        self.world.work
            + if observation_is_empty {
                0
            } else {
                actors as u64 * 2
            }
    }

    fn summary(&self) -> BTreeMap<&'static str, u64> {
        let mut result = BTreeMap::new();
        let (visited, required) = self.world.summary();
        result.insert("visited_patrol_points", visited);
        result.insert("required_patrol_points", required);
        result
    }

    fn expected_output(&self) -> Option<&[u8]> {
        None
    }

    fn actual_output(&self) -> Option<&[u8]> {
        None
    }

    fn rounds(&self) -> u64 {
        self.world.rounds
    }

    fn debug_snapshot(&mut self, actor: usize) -> Option<ActorSnapshot> {
        Some(self.world.debug_snapshot(actor))
    }

    fn round_changes(&mut self) -> Result<Vec<SceneRoundChange>, SceneCounterError> {
        Ok(self.world.round_changes())
    }

    fn end_round(&mut self, _observation: Vec<u8>) -> Result<Vec<u8>, SceneCounterError> {
        self.world
            .end_round()
            .map_err(|_| SceneCounterError::Overflow)
    }
}

#[derive(Clone)]
pub(super) struct MechanicalArmRuntime {
    world: ArmWorld,
}

impl MechanicalArmRuntime {
    pub fn new(
        actors: u8,
        tables: [Option<Worktable>; 4],
        input: &[InputRobot],
        expected: &[u8],
    ) -> Self {
        Self {
            world: ArmWorld::new(actors, tables, input, expected),
        }
    }
}

impl SceneRuntime for MechanicalArmRuntime {
    fn kind(&self) -> SceneKind {
        SceneKind::MechanicalArm
    }

    fn mode(&self) -> SceneRuntimeMode {
        SceneRuntimeMode::ActorFrame
    }

    fn reset_transition_work(&mut self) {
        self.world.work = 0;
    }

    fn transition(
        &mut self,
        command: SceneCommand,
    ) -> Result<RuntimeTransition, SceneCounterError> {
        let SceneCommand::ActorOutput { actor, action } = command else {
            unreachable!("mechanical-arm runtime only receives actor outputs")
        };
        let result = match self.world.act(actor, action) {
            Ok(observation) => RuntimeTransition {
                outcome: self.world.complete().then_some(SceneOutcome::Passed),
                observation,
            },
            Err(ArmError::CounterOverflow) => return Err(SceneCounterError::Overflow),
            Err(ArmError::InvalidOutput) => RuntimeTransition {
                outcome: Some(SceneOutcome::Failed(SceneFailure::InvalidOutput {
                    actor: Some(actor as u8),
                    value: action,
                })),
                observation: Vec::new(),
            },
            Err(ArmError::WrongOutput {
                index,
                expected,
                actual,
            }) => RuntimeTransition {
                outcome: Some(SceneOutcome::Failed(SceneFailure::WrongOutput {
                    actor: Some(actor as u8),
                    index,
                    expected,
                    actual,
                })),
                observation: Vec::new(),
            },
            Err(error) => {
                let operation = match error {
                    ArmError::OccupiedHand => IllegalOperationReason::OccupiedHand,
                    ArmError::OccupiedTarget => IllegalOperationReason::OccupiedTarget,
                    ArmError::InvalidDropTarget => IllegalOperationReason::InvalidDropTarget,
                    ArmError::InvalidTableInput => IllegalOperationReason::InvalidTableInput,
                    _ => unreachable!("handled arm error"),
                };
                RuntimeTransition {
                    outcome: Some(SceneOutcome::Failed(SceneFailure::IllegalOperation {
                        actor: actor as u8,
                        operation,
                        interaction: self.world.public_interaction(actor),
                    })),
                    observation: Vec::new(),
                }
            }
        };
        Ok(result)
    }

    fn record_action_feedback_work(&mut self) {
        self.world.work += 1;
    }

    fn take_initial_input(&mut self) -> Vec<u8> {
        self.world.initial_input()
    }

    fn initialization_work(&self, actors: usize, initial_input_len: usize) -> u64 {
        self.world.work
            + if initial_input_len == 0 {
                0
            } else {
                actors as u64
            }
    }

    fn retained_units(&self) -> u64 {
        self.world.retained_units()
    }

    fn transition_work(&self, actors: usize, observation_is_empty: bool) -> u64 {
        self.world.work
            + if observation_is_empty {
                0
            } else {
                actors as u64
            }
    }

    fn summary(&self) -> BTreeMap<&'static str, u64> {
        let mut result = BTreeMap::new();
        result.insert("matched_output_robots", self.world.matched);
        result
    }

    fn expected_output(&self) -> Option<&[u8]> {
        Some(self.world.expected_output())
    }

    fn actual_output(&self) -> Option<&[u8]> {
        Some(&self.world.actual)
    }

    fn rounds(&self) -> u64 {
        self.world.rounds
    }

    fn debug_snapshot(&mut self, actor: usize) -> Option<ActorSnapshot> {
        Some(self.world.debug_snapshot(actor))
    }

    fn round_changes(&mut self) -> Result<Vec<SceneRoundChange>, SceneCounterError> {
        self.world
            .round_changes()
            .map_err(|_| SceneCounterError::Overflow)
    }

    fn end_round(&mut self, observation: Vec<u8>) -> Result<Vec<u8>, SceneCounterError> {
        self.world
            .end_round(observation)
            .map_err(|_| SceneCounterError::Overflow)
    }
}
