//! Deterministic scene transitions. VM stepping and host limits live above this layer.
use crate::schema::{MapCell, RobotStart};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionError {
    InvalidOutput,
    CounterOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RobotWorld {
    pub robots: Vec<RobotStart>,
    map: Vec<MapCell>,
    visited: BTreeSet<u8>,
    triggered: BTreeSet<u8>,
    pending: BTreeSet<u8>,
    pub open_doors: BTreeSet<u8>,
    required: u64,
    map_units: u64,
    pub rounds: u64,
    pub work: u64,
}
impl RobotWorld {
    pub fn new(robots: &[RobotStart], map: &[MapCell]) -> Self {
        let mut world = Self {
            robots: robots.to_vec(),
            map: map.to_vec(),
            visited: BTreeSet::new(),
            triggered: BTreeSet::new(),
            pending: BTreeSet::new(),
            open_doors: BTreeSet::new(),
            required: 0,
            map_units: map.len() as u64,
            rounds: 0,
            work: 0,
        };
        for cell in map {
            world.work += 1;
            world.required += u64::from(cell.patrol);
            world.map_units += u64::from(cell.trigger.is_some()) + u64::from(cell.door.is_some());
        }
        for robot in robots {
            world.enter(robot.position);
        }
        world
    }
    fn enter(&mut self, position: u8) {
        self.work += 1;
        let cell = self.map[usize::from(position)];
        if cell.patrol {
            self.work += 1;
            if self.visited.insert(position) {
                self.work += 1;
            }
        }
        if let Some(trigger) = cell.trigger {
            self.work += 2; // Trigger association and triggered-set lookup.
            if self.triggered.insert(trigger.id) {
                self.work += 2; // New trigger entry and pending-door entry.
                self.pending.insert(trigger.door_id);
            }
        }
    }
    pub fn complete(&mut self) -> bool {
        self.visited.len() as u64 == self.required
    }
    pub fn observation(&self) -> Vec<u8> {
        self.robots
            .iter()
            .flat_map(|robot| [robot.position, self.map[usize::from(robot.position)].color])
            .collect()
    }
    pub fn act(&mut self, actor: usize, action: u8) -> Result<(), TransitionError> {
        if action > 4 {
            return Err(TransitionError::InvalidOutput);
        }
        self.work += 1; // Actor inspected.
        match action {
            0 => {}
            3 => {
                self.work += 1;
                self.robots[actor].direction = (self.robots[actor].direction + 3) % 4;
            }
            4 => {
                self.work += 1;
                self.robots[actor].direction = (self.robots[actor].direction + 1) % 4;
            }
            1 | 2 => {
                let robot = self.robots[actor];
                let x = i16::from(robot.position % 16);
                let y = i16::from(robot.position / 16);
                let (dx, dy) = [(0, -1), (1, 0), (0, 1), (-1, 0)][usize::from(robot.direction)];
                let (x, y) = (x + dx, y + dy);
                if !(0..16).contains(&x) || !(0..16).contains(&y) {
                    return Ok(());
                }
                let target = (y * 16 + x) as u8;
                self.work += 2; // Target and current cell.
                let cell = self.map[usize::from(target)];
                let same_height = cell.height == self.map[usize::from(robot.position)].height;
                if !cell.walkable {
                    return Ok(());
                }
                if let Some(id) = cell.door {
                    self.work += 1;
                    if !self.open_doors.contains(&id) {
                        return Ok(());
                    }
                }
                for other in &self.robots {
                    self.work += 1;
                    if other.position == target {
                        return Ok(());
                    }
                }
                if same_height != (action == 1) {
                    return Ok(());
                }
                self.work += 1;
                self.robots[actor].position = target;
                self.enter(target);
            }
            _ => unreachable!("bounded action"),
        }
        Ok(())
    }
    /// Opening doors is deferred until all actors complete a nonterminal frame.
    pub fn end_round(&mut self) -> Result<Vec<u8>, TransitionError> {
        self.rounds = self
            .rounds
            .checked_add(1)
            .ok_or(TransitionError::CounterOverflow)?;
        self.work += self.pending.len() as u64 * 2;
        self.open_doors.append(&mut self.pending);
        Ok(self.observation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn map() -> Vec<MapCell> {
        let mut map = vec![
            MapCell {
                color: 0,
                height: 0,
                walkable: true,
                patrol: false,
                door: None,
                trigger: None
            };
            256
        ];
        map[255].patrol = true;
        map
    }
    #[test]
    fn robot_geometry_jump_turns_and_boundaries() {
        let mut cells = map();
        cells[34].color = 2;
        cells[18].height = 1;
        let mut world = RobotWorld::new(
            &[RobotStart {
                position: 50,
                direction: 0,
            }],
            &cells,
        );
        world.act(0, 1).unwrap();
        assert_eq!(world.observation(), [34, 2]);
        world.act(0, 1).unwrap();
        assert_eq!(world.robots[0].position, 34);
        world.act(0, 2).unwrap();
        assert_eq!(world.robots[0].position, 18);
        world.act(0, 3).unwrap();
        assert_eq!(world.robots[0].direction, 3);
        world.act(0, 4).unwrap();
        assert_eq!(world.robots[0].direction, 0);
        world.robots[0].position = 0;
        world.act(0, 1).unwrap();
        assert_eq!(world.robots[0].position, 0);
        assert_eq!(world.act(0, 5), Err(TransitionError::InvalidOutput));
    }
    #[test]
    fn robot_ordered_occupancy_and_delayed_door() {
        let mut cells = map();
        cells[1].trigger = Some(crate::Trigger { id: 4, door_id: 7 });
        cells[2].door = Some(7);
        let mut world = RobotWorld::new(
            &[
                RobotStart {
                    position: 0,
                    direction: 1,
                },
                RobotStart {
                    position: 3,
                    direction: 3,
                },
            ],
            &cells,
        );
        world.act(0, 1).unwrap();
        world.act(1, 1).unwrap();
        assert_eq!(
            world.robots.iter().map(|r| r.position).collect::<Vec<_>>(),
            [1, 3]
        );
        assert!(world.open_doors.is_empty());
        world.end_round().unwrap();
        assert!(world.open_doors.contains(&7));
        world.act(1, 1).unwrap();
        assert_eq!(world.robots[1].position, 2);
        world.act(0, 1).unwrap();
        world.act(1, 1).unwrap();
        assert_eq!(
            world.robots.iter().map(|r| r.position).collect::<Vec<_>>(),
            [1, 2]
        );
    }
    #[test]
    fn robot_initial_patrol_and_initial_trigger() {
        let mut cells = map();
        cells[255].trigger = Some(crate::Trigger { id: 1, door_id: 8 });
        cells[0].door = Some(8);
        let mut world = RobotWorld::new(
            &[RobotStart {
                position: 255,
                direction: 0,
            }],
            &cells,
        );
        assert!(world.complete());
        assert!(world.open_doors.is_empty());
        world.end_round().unwrap();
        assert!(world.open_doors.contains(&8));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ArmRobot {
    state: u8,
    true_inspection: u8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotState {
    Empty,
    Pending(ArmRobot),
    Ready(ArmRobot),
}
impl SlotState {
    fn occupied(self) -> bool {
        !matches!(self, Self::Empty)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArmTarget {
    Input,
    Output,
    Table(usize),
    Buffer(usize),
    Unavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArmError {
    InvalidOutput,
    OccupiedHand,
    OccupiedTarget,
    InvalidDropTarget,
    InvalidTableInput,
    WrongOutput {
        index: usize,
        expected: Option<u8>,
        actual: u8,
    },
    CounterOverflow,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArmWorld {
    pub orientations: Vec<u8>,
    hands: Vec<Option<ArmRobot>>,
    input: std::collections::VecDeque<ArmRobot>,
    tables: [Option<crate::Worktable>; 4],
    slots: [SlotState; 4],
    buffers: [SlotState; 2],
    expected: Vec<u8>,
    pub actual: Vec<u8>,
    pub matched: u64,
    robot_count: usize,
    pub rounds: u64,
    pub work: u64,
}
impl ArmWorld {
    pub fn new(
        actors: u8,
        tables: [Option<crate::Worktable>; 4],
        input: &[crate::InputRobot],
        expected: &[u8],
    ) -> Self {
        Self {
            orientations: vec![0; usize::from(actors)],
            hands: vec![None; usize::from(actors)],
            input: input
                .iter()
                .map(|robot| ArmRobot {
                    state: 12 | robot.color,
                    true_inspection: robot.true_inspection,
                })
                .collect(),
            tables,
            slots: [SlotState::Empty; 4],
            buffers: [SlotState::Empty; 2],
            expected: expected.to_vec(),
            actual: Vec::new(),
            matched: 0,
            robot_count: input.len(),
            rounds: 0,
            work: 0,
        }
    }
    pub fn complete(&self) -> bool {
        self.actual.len() == self.expected.len()
    }
    pub fn observation(&self) -> Vec<u8> {
        self.hands
            .iter()
            .map(|hand| hand.map_or(255, |robot| robot.state))
            .collect()
    }
    pub fn initial_input(&self) -> Vec<u8> {
        if self.hands.len() == 2 {
            self.observation()
        } else {
            Vec::new()
        }
    }
    fn target(&self, actor: usize) -> ArmTarget {
        let orientation = self.orientations[actor];
        if orientation == 3 {
            return ArmTarget::Output;
        }
        if self.hands.len() == 1 {
            return match orientation {
                0 => ArmTarget::Input,
                1 => ArmTarget::Table(0),
                2 => ArmTarget::Table(1),
                4 => ArmTarget::Table(2),
                5 => ArmTarget::Table(3),
                _ => ArmTarget::Unavailable,
            };
        }
        match (actor, orientation) {
            (0, 0) => ArmTarget::Input,
            (0, 1) => ArmTarget::Table(0),
            (0, 2) => ArmTarget::Table(1),
            (0, 4) | (1, 2) => ArmTarget::Buffer(1),
            (0, 5) | (1, 1) => ArmTarget::Buffer(0),
            (1, 4) => ArmTarget::Table(3),
            (1, 5) => ArmTarget::Table(2),
            _ => ArmTarget::Unavailable,
        }
    }
    fn effective_target(&self, actor: usize) -> ArmTarget {
        match self.target(actor) {
            ArmTarget::Table(index) if self.tables[index].is_none() => ArmTarget::Unavailable,
            target => target,
        }
    }
    fn transform(table: crate::Worktable, mut robot: ArmRobot) -> Result<ArmRobot, ArmError> {
        use crate::Worktable::*;
        let inspection = (robot.state >> 2) & 3;
        let processed = robot.state & 16 != 0;
        let packed = robot.state & 32 != 0;
        let valid = match table {
            Inspection => inspection == 3 && !processed && !packed,
            Repair => inspection == 1 && !processed && !packed,
            Processing => inspection == 0 && !processed && !packed,
            Packing => inspection == 0 && !packed,
            Removal => true,
        };
        if !valid {
            return Err(ArmError::InvalidTableInput);
        }
        match table {
            Inspection => robot.state = (robot.state & 3) | (robot.true_inspection << 2),
            Repair => robot.state &= !12,
            Processing => robot.state |= 16,
            Packing => robot.state |= 32,
            Removal => {}
        }
        Ok(robot)
    }
    /// Return single-arm event input; publication is deferred to the session.
    pub fn act(&mut self, actor: usize, action: u8) -> Result<Vec<u8>, ArmError> {
        if action > 4 {
            return Err(ArmError::InvalidOutput);
        }
        self.work += 1; // Actor inspected.
        if action == 0 {
            return Ok(Vec::new());
        }
        if action == 3 || action == 4 {
            self.work += 1;
            self.orientations[actor] =
                (self.orientations[actor] + if action == 3 { 5 } else { 1 }) % 6;
            return Ok(Vec::new());
        }
        self.work += 1; // Target slot/input inspected.
        let target = self.effective_target(actor);
        if action == 1 {
            let occupied = match target {
                ArmTarget::Input => !self.input.is_empty(),
                ArmTarget::Table(index) => self.slots[index].occupied(),
                ArmTarget::Buffer(index) => self.buffers[index].occupied(),
                _ => false,
            };
            if self.hands[actor].is_some() {
                return if occupied {
                    Err(ArmError::OccupiedHand)
                } else {
                    Ok(Vec::new())
                };
            }
            let acquired = match target {
                ArmTarget::Input => self.input.pop_front(),
                ArmTarget::Table(index) => match self.slots[index] {
                    SlotState::Ready(robot) => {
                        self.slots[index] = SlotState::Empty;
                        Some(robot)
                    }
                    _ => None,
                },
                ArmTarget::Buffer(index) => match self.buffers[index] {
                    SlotState::Ready(robot) => {
                        self.buffers[index] = SlotState::Empty;
                        Some(robot)
                    }
                    _ => None,
                },
                _ => None,
            };
            if acquired.is_some() {
                self.work += 3;
            } // Robot inspected, source and hand updated.
            self.hands[actor] = acquired;
            return Ok(if self.hands.len() == 1 {
                acquired.into_iter().map(|r| r.state).collect()
            } else {
                Vec::new()
            });
        }
        let Some(robot) = self.hands[actor] else {
            return Ok(Vec::new());
        };
        self.work += 1; // Held robot inspected.
        match target {
            ArmTarget::Input | ArmTarget::Unavailable => return Err(ArmError::InvalidDropTarget),
            ArmTarget::Output => {
                let index = self.actual.len();
                let expected = self.expected.get(index).copied();
                self.work += 3; // Expected inspected, output and hand updated.
                self.robot_count -= 1;
                self.actual.push(robot.state);
                self.hands[actor] = None;
                if expected != Some(robot.state) {
                    return Err(ArmError::WrongOutput {
                        index,
                        expected,
                        actual: robot.state,
                    });
                }
                self.matched += 1;
            }
            ArmTarget::Buffer(index) => {
                if self.buffers[index].occupied() {
                    return Err(ArmError::OccupiedTarget);
                }
                self.work += 2;
                self.buffers[index] = SlotState::Pending(robot);
                self.hands[actor] = None;
            }
            ArmTarget::Table(index) => {
                if self.slots[index].occupied() {
                    return Err(ArmError::OccupiedTarget);
                }
                let table = self.tables[index].expect("effective target has table");
                if table != crate::Worktable::Removal {
                    Self::transform(table, robot)?;
                    self.slots[index] = SlotState::Pending(robot);
                } else {
                    self.robot_count -= 1;
                }
                self.work += 2; // Table/Removal and hand updated.
                self.hands[actor] = None;
            }
        }
        Ok(Vec::new())
    }
    pub fn end_round(&mut self, single_event: Vec<u8>) -> Result<Vec<u8>, ArmError> {
        let rounds = self
            .rounds
            .checked_add(1)
            .ok_or(ArmError::CounterOverflow)?;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            self.work += 1;
            if let SlotState::Pending(robot) = *slot {
                self.work += 3; // Table/robot inspected and completed slot updated.
                *slot = SlotState::Ready(Self::transform(
                    self.tables[index].expect("occupied table exists"),
                    robot,
                )?);
            }
        }
        for slot in &mut self.buffers {
            self.work += 1;
            if let SlotState::Pending(robot) = *slot {
                self.work += 1;
                *slot = SlotState::Ready(robot);
            }
        }
        self.rounds = rounds;
        Ok(if self.hands.len() == 2 {
            self.observation()
        } else {
            single_event
        })
    }
}

#[cfg(test)]
mod arm_tests {
    use super::*;
    use crate::{InputRobot, Worktable};
    fn arm(actors: u8) -> ArmWorld {
        ArmWorld::new(
            actors,
            [
                Some(Worktable::Inspection),
                Some(Worktable::Packing),
                Some(Worktable::Repair),
                Some(Worktable::Removal),
            ],
            &[InputRobot {
                color: 1,
                true_inspection: 0,
            }],
            &[33],
        )
    }
    fn round(world: &mut ArmWorld, action: u8) -> Vec<u8> {
        let event = world.act(0, action).unwrap();
        world.end_round(event).unwrap()
    }
    #[test]
    fn inspection_direct_pack_and_output_fifo() {
        let mut world = arm(1);
        assert!(world.initial_input().is_empty());
        assert_eq!(round(&mut world, 1), [13]);
        round(&mut world, 4);
        round(&mut world, 2);
        assert!(world.hands[0].is_none());
        assert_eq!(round(&mut world, 1), [1]);
        round(&mut world, 4);
        round(&mut world, 2);
        assert_eq!(round(&mut world, 1), [33]);
        round(&mut world, 4);
        world.act(0, 2).unwrap();
        assert!(world.complete());
        assert_eq!(world.actual, [33]);
    }
    #[test]
    fn pending_buffer_cannot_be_grabbed_same_round_and_keeps_identity() {
        let mut world = arm(2);
        assert_eq!(world.initial_input(), [255, 255]);
        world.act(0, 1).unwrap();
        world.end_round(Vec::new()).unwrap();
        world.orientations = [5, 1].to_vec();
        world.act(0, 2).unwrap();
        world.act(1, 1).unwrap();
        assert_eq!(world.observation(), [255, 255]);
        world.end_round(Vec::new()).unwrap();
        world.act(1, 1).unwrap();
        assert_eq!(world.observation(), [255, 13]);
        assert_eq!(world.hands[1].unwrap().true_inspection, 0);
    }
    #[test]
    fn table_preconditions_and_removal() {
        let uninspected = ArmRobot {
            state: 13,
            true_inspection: 1,
        };
        let defect = ArmWorld::transform(Worktable::Inspection, uninspected).unwrap();
        assert_eq!(defect.state, 5);
        let normal = ArmWorld::transform(Worktable::Repair, defect).unwrap();
        assert_eq!(normal.state, 1);
        let processed = ArmWorld::transform(Worktable::Processing, normal).unwrap();
        assert_eq!(processed.state, 17);
        assert_eq!(
            ArmWorld::transform(Worktable::Packing, processed)
                .unwrap()
                .state,
            49
        );
        for (table, robot) in [
            (Worktable::Inspection, normal),
            (Worktable::Repair, normal),
            (Worktable::Processing, processed),
            (Worktable::Packing, defect),
        ] {
            assert_eq!(
                ArmWorld::transform(table, robot),
                Err(ArmError::InvalidTableInput)
            );
        }
        let mut world = arm(1);
        round(&mut world, 1);
        world.orientations[0] = 5;
        round(&mut world, 2);
        assert!(world.hands[0].is_none());
        assert_eq!(world.slots[3], SlotState::Empty);
    }
    #[test]
    fn grab_drop_occupancy_and_invalid_target_rules() {
        let mut world = arm(1);
        world.act(0, 2).unwrap();
        world.act(0, 1).unwrap();
        assert_eq!(world.act(0, 2), Err(ArmError::InvalidDropTarget));
        world.input.push_back(ArmRobot {
            state: 12,
            true_inspection: 2,
        });
        assert_eq!(world.act(0, 1), Err(ArmError::OccupiedHand));
        world.orientations[0] = 1;
        world.slots[0] = SlotState::Pending(ArmRobot {
            state: 12,
            true_inspection: 2,
        });
        assert_eq!(world.act(0, 1), Err(ArmError::OccupiedHand));
        assert_eq!(world.act(0, 2), Err(ArmError::OccupiedTarget));
        world.hands[0] = None;
        assert!(world.act(0, 1).unwrap().is_empty());
        world.orientations[0] = 3;
        world.hands[0] = Some(ArmRobot {
            state: 13,
            true_inspection: 0,
        });
        assert_eq!(
            world.act(0, 2),
            Err(ArmError::WrongOutput {
                index: 0,
                expected: Some(33),
                actual: 13
            })
        );
        assert!(world.hands[0].is_none());
        assert_eq!(world.actual, [13]);
    }
}

impl RobotWorld {
    pub fn retained_units(&self) -> u64 {
        self.map_units
            + (self.robots.len()
                + self.visited.len()
                + self.triggered.len()
                + self.pending.len()
                + self.open_doors.len()) as u64
    }
    pub fn summary(&self) -> (u64, u64) {
        (self.visited.len() as u64, self.required)
    }
}
impl ArmWorld {
    pub fn expected_output(&self) -> &[u8] {
        &self.expected
    }
    pub fn retained_units(&self) -> u64 {
        (self.hands.len() + self.robot_count + 4 + 2 + self.expected.len() + self.actual.len())
            as u64
    }
}

pub(crate) enum ActorSnapshot {
    Robot {
        position: u8,
        direction: u8,
    },
    Arm {
        orientation: u8,
        state: u8,
        interaction: crate::scene_feedback::SceneInteraction,
    },
}
impl ActorSnapshot {
    pub fn effect(self, after: Self) -> crate::scene_feedback::SceneEffect {
        use crate::scene_feedback::SceneEffect;
        match (self, after) {
            (
                Self::Robot {
                    position: from_position,
                    direction: from_direction,
                },
                Self::Robot {
                    position: to_position,
                    direction: to_direction,
                },
            ) => SceneEffect::Robot {
                from_position,
                to_position,
                from_direction,
                to_direction,
            },
            (
                Self::Arm {
                    orientation: from_orientation,
                    state: before_state,
                    interaction,
                },
                Self::Arm {
                    orientation: to_orientation,
                    state: after_state,
                    ..
                },
            ) => SceneEffect::MechanicalArm {
                interaction,
                from_orientation,
                to_orientation,
                before_state,
                after_state,
            },
            _ => unreachable!("actor snapshot belongs to unchanged world kind"),
        }
    }
}
impl RobotWorld {
    pub fn debug_snapshot(&mut self, actor: usize) -> ActorSnapshot {
        self.work += 1;
        let robot = self.robots[actor];
        ActorSnapshot::Robot {
            position: robot.position,
            direction: robot.direction,
        }
    }
    pub fn round_changes(&mut self) -> Vec<crate::scene_feedback::SceneRoundChange> {
        self.work += self.pending.len() as u64;
        self.pending
            .iter()
            .map(|id| crate::scene_feedback::SceneRoundChange::DoorOpened { door_id: *id })
            .collect()
    }
}
impl ArmWorld {
    pub fn public_interaction(&self, actor: usize) -> crate::scene_feedback::SceneInteraction {
        use crate::scene_feedback::SceneInteraction as I;
        match self.target(actor) {
            ArmTarget::Input => I::Input,
            ArmTarget::Output => I::Output,
            ArmTarget::Table(index) => I::Worktable(index),
            ArmTarget::Buffer(0) => I::BufferLeft,
            ArmTarget::Buffer(_) => I::BufferRight,
            ArmTarget::Unavailable => I::Unavailable,
        }
    }
    pub fn debug_snapshot(&mut self, actor: usize) -> ActorSnapshot {
        self.work += 1 + u64::from(self.hands[actor].is_some());
        let interaction = self.public_interaction(actor);
        if matches!(
            interaction,
            crate::scene_feedback::SceneInteraction::Worktable(_)
        ) {
            self.work += 1;
        }
        ActorSnapshot::Arm {
            orientation: self.orientations[actor],
            state: self.hands[actor].map_or(255, |r| r.state),
            interaction,
        }
    }
    pub fn round_changes(
        &mut self,
    ) -> Result<Vec<crate::scene_feedback::SceneRoundChange>, ArmError> {
        use crate::scene_feedback::SceneRoundChange;
        let mut changes = Vec::new();
        for (index, slot) in self.slots.iter().enumerate() {
            self.work += 1;
            if let SlotState::Pending(robot) = slot {
                self.work += 3;
                let robot = Self::transform(
                    self.tables[index].expect("pending table configured"),
                    *robot,
                )?;
                changes.push(SceneRoundChange::TableReady {
                    worktable_index: index,
                    state: robot.state,
                });
            }
        }
        for (index, slot) in self.buffers.iter().enumerate() {
            self.work += 1;
            if let SlotState::Pending(robot) = slot {
                self.work += 1;
                changes.push(SceneRoundChange::BufferReady {
                    right: index == 1,
                    state: robot.state,
                });
            }
        }
        Ok(changes)
    }
}
