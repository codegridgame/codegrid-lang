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
            .flat_map(|robot| [self.map[usize::from(robot.position)].color, robot.position])
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
        assert_eq!(world.observation(), [2, 34]);
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

fn push_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len as u64).to_le_bytes());
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
    /// Deterministic semantic encoding for exact cycle detection. Excludes the
    /// immutable map and the monotone round/work counters.
    pub(crate) fn write_semantic_state(&self, out: &mut Vec<u8>) {
        push_len(out, self.robots.len());
        for robot in &self.robots {
            out.push(robot.position);
            out.push(robot.direction);
        }
        for set in [
            &self.visited,
            &self.triggered,
            &self.pending,
            &self.open_doors,
        ] {
            push_len(out, set.len());
            out.extend(set.iter().copied());
        }
    }
    /// Monotone goal progress: visited patrol points.
    pub(crate) fn progress_counter(&self) -> u64 {
        self.visited.len() as u64
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArmError {
    InvalidOutput,
    OccupiedHand,
    OccupiedTarget,
    InvalidDropTarget,
    HeavyOnLight,
    SharedStationConflict,
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
    hands: Vec<Option<u8>>,
    stations: Vec<crate::StationType>,
    contents: Vec<Vec<u8>>,
    input: std::collections::VecDeque<u8>,
    mode: crate::MechanicalArmMode,
    expected: Vec<u8>,
    pub actual: Vec<u8>,
    pub matched: u64,
    pub rounds: u64,
    pub work: u64,
}
impl ArmWorld {
    pub fn new(
        actors: u8,
        mode: crate::MechanicalArmMode,
        stations: &[crate::StationType],
        initial: &[Vec<u8>],
        input: &[u8],
        expected: &[u8],
    ) -> Self {
        Self {
            orientations: vec![0; actors as usize],
            hands: vec![None; actors as usize],
            stations: stations.to_vec(),
            contents: initial.to_vec(),
            input: input.iter().copied().collect(),
            mode,
            expected: expected.to_vec(),
            actual: Vec::new(),
            matched: 0,
            rounds: 0,
            work: initial.iter().map(|s| s.len() as u64 + 1).sum::<u64>()
                + stations.len() as u64
                + input.len() as u64
                + expected.len() as u64,
        }
    }
    pub fn target(&self, actor: usize) -> usize {
        if actor == 0 {
            self.orientations[actor] as usize
        } else {
            [6, 5, 4, 7, 8, 9][self.orientations[actor] as usize]
        }
    }
    pub fn preflight(&mut self, actions: &[u8]) -> Option<(usize, ArmError)> {
        for (actor, action) in actions.iter().enumerate() {
            self.work += 1;
            if *action > 4 {
                return Some((actor, ArmError::InvalidOutput));
            }
        }
        if actions.len() == 2
            && matches!(actions[0], 1 | 2)
            && matches!(actions[1], 1 | 2)
            && self.target(0) == self.target(1)
        {
            return Some((1, ArmError::SharedStationConflict));
        }
        None
    }
    pub fn complete(&mut self) -> bool {
        if self.mode == crate::MechanicalArmMode::FinalLayout {
            self.work += self
                .contents
                .iter()
                .map(|s| s.len() as u64 + 1)
                .sum::<u64>();
            let matched = serialized_layout_matches(&self.contents, &self.expected);
            if matched {
                self.work += self.expected.len() as u64;
                self.actual.clone_from(&self.expected);
            }
            matched
        } else {
            self.actual.len() == self.expected.len()
        }
    }
    pub fn halt_complete(&mut self) -> bool {
        if self.mode == crate::MechanicalArmMode::FinalLayout {
            self.actual = self.layout();
            self.actual == self.expected
        } else {
            self.complete()
        }
    }
    pub fn mode_is_final_layout(&self) -> bool {
        self.mode == crate::MechanicalArmMode::FinalLayout
    }
    pub fn frame_peak_growth_units(&self, actors: usize) -> u64 {
        if self.mode == crate::MechanicalArmMode::FinalLayout {
            self.expected.len() as u64
        } else {
            self.expected
                .len()
                .saturating_sub(self.actual.len())
                .min(actors) as u64
        }
    }
    fn layout(&self) -> Vec<u8> {
        self.contents
            .iter()
            .flat_map(|s| std::iter::once(s.len() as u8).chain(s.iter().copied()))
            .collect()
    }
    pub fn observation(&self) -> Vec<u8> {
        self.hands.iter().map(|h| h.unwrap_or(255)).collect()
    }
    pub fn initial_input(&self) -> Vec<u8> {
        self.observation()
    }
    pub fn act(&mut self, actor: usize, action: u8) -> Result<Vec<u8>, ArmError> {
        use crate::StationType;
        self.work += 1;
        if action > 4 {
            return Err(ArmError::InvalidOutput);
        }
        if action == 0 {
            return Ok(Vec::new());
        }
        if action == 3 || action == 4 {
            self.orientations[actor] =
                (self.orientations[actor] + if action == 3 { 5 } else { 1 }) % 6;
            self.work += 1;
            return Ok(Vec::new());
        }
        let target = self.target(actor);
        let kind = self.stations[target];
        self.work += 1;
        if action == 1 {
            let available = if kind == StationType::Input {
                !self.input.is_empty()
            } else if kind == StationType::Output {
                false
            } else {
                !self.contents[target].is_empty()
            };
            if !available {
                return Ok(Vec::new());
            }
            if self.hands[actor].is_some() {
                return Err(ArmError::OccupiedHand);
            }
            self.hands[actor] = if kind == StationType::Input {
                self.input.pop_front()
            } else {
                self.contents[target].pop()
            };
            self.work += 2;
            return Ok(Vec::new());
        }
        let Some(item) = self.hands[actor] else {
            return Ok(Vec::new());
        };
        match kind {
            StationType::Input => return Err(ArmError::InvalidDropTarget),
            StationType::Output => {
                let index = self.actual.len();
                let expected = self.expected.get(index).copied();
                self.work += 1;
                if expected != Some(item) {
                    return Err(ArmError::WrongOutput {
                        index,
                        expected,
                        actual: item,
                    });
                }
                let matched = self
                    .matched
                    .checked_add(1)
                    .ok_or(ArmError::CounterOverflow)?;
                self.actual.push(item);
                self.matched = matched;
            }
            StationType::Stack => {
                if self.contents[target].len() == 4 {
                    return Err(ArmError::OccupiedTarget);
                }
                if item & 8 != 0
                    && self.contents[target]
                        .last()
                        .is_some_and(|below| below & 8 == 0)
                {
                    return Err(ArmError::HeavyOnLight);
                }
                self.contents[target].push(item);
            }
            StationType::Flip => {
                if !self.contents[target].is_empty() {
                    return Err(ArmError::OccupiedTarget);
                }
                self.contents[target].push(item ^ 4);
            }
        }
        self.hands[actor] = None;
        self.work += 2;
        Ok(Vec::new())
    }
    pub fn end_round(&mut self, _: Vec<u8>) -> Result<Vec<u8>, ArmError> {
        self.rounds = self
            .rounds
            .checked_add(1)
            .ok_or(ArmError::CounterOverflow)?;
        Ok(self.observation())
    }
    pub fn expected_output(&self) -> &[u8] {
        &self.expected
    }
    pub fn retained_units(&self) -> u64 {
        (self.orientations.len()
            + self.hands.len()
            + self.stations.len()
            + self.contents.len()
            + self.contents.iter().map(Vec::len).sum::<usize>()
            + self.input.len()
            + self.expected.len()
            + self.actual.len()) as u64
    }
    pub fn write_semantic_state(&self, out: &mut Vec<u8>) {
        out.extend(&self.orientations);
        out.extend(self.observation());
        push_len(out, self.input.len());
        out.extend(&self.input);
        out.extend(self.layout());
        push_len(out, self.actual.len());
        out.extend(&self.actual);
    }
    pub fn progress_counter(&self) -> u64 {
        self.matched
    }
    pub fn public_interaction(&self, actor: usize) -> crate::scene_feedback::SceneInteraction {
        crate::scene_feedback::SceneInteraction::Station(self.target(actor))
    }
    pub fn debug_snapshot(&mut self, actor: usize) -> ActorSnapshot {
        self.work += 1;
        ActorSnapshot::Arm {
            orientation: self.orientations[actor],
            state: self.hands[actor].unwrap_or(255),
            interaction: self.public_interaction(actor),
        }
    }
    pub fn round_changes(
        &mut self,
    ) -> Result<Vec<crate::scene_feedback::SceneRoundChange>, ArmError> {
        self.work += self
            .contents
            .iter()
            .map(|s| 1 + s.len() as u64)
            .sum::<u64>();
        Ok(self
            .contents
            .iter()
            .enumerate()
            .map(
                |(station_id, contents)| crate::scene_feedback::SceneRoundChange::StationUpdated {
                    station_id,
                    contents: contents.clone(),
                },
            )
            .collect())
    }
}

pub(crate) fn serialized_layout_matches(contents: &[Vec<u8>], expected: &[u8]) -> bool {
    contents
        .iter()
        .flat_map(|stack| std::iter::once(stack.len() as u8).chain(stack.iter().copied()))
        .eq(expected.iter().copied())
}

#[cfg(test)]
mod arm_redesign_tests {
    use super::*;
    use crate::{MechanicalArmMode as Mode, StationType as Station};
    fn world(stations: Vec<Station>, initial: Vec<Vec<u8>>) -> ArmWorld {
        ArmWorld::new(1, Mode::FinalLayout, &stations, &initial, &[], &[])
    }
    #[test]
    fn stack_grab_is_lifo_and_preserves_full_byte() {
        let mut arm = world(
            vec![Station::Stack; 6],
            vec![vec![9, 17], vec![], vec![], vec![], vec![], vec![]],
        );
        assert_eq!(arm.initial_input(), [255]);
        arm.act(0, 1).unwrap();
        assert_eq!(arm.observation(), [17]);
        assert_eq!(arm.contents[0], [9]);
        assert_eq!(arm.act(0, 1), Err(ArmError::OccupiedHand));
        arm.act(0, 4).unwrap();
        arm.act(0, 2).unwrap();
        assert_eq!(arm.contents[1], [17]);
    }
    #[test]
    fn flip_is_immediate_and_cannot_make_empty_sentinel() {
        for byte in 0u8..=254 {
            if byte & 3 == 3 {
                continue;
            }
            let mut arm = world(
                vec![Station::Flip; 6],
                vec![vec![], vec![], vec![], vec![], vec![], vec![]],
            );
            arm.hands[0] = Some(byte);
            arm.act(0, 2).unwrap();
            assert_eq!(arm.contents[0], [byte ^ 4]);
            arm.act(0, 1).unwrap();
            assert_eq!(arm.observation(), [byte ^ 4]);
            assert_ne!(byte ^ 4, 255);
        }
    }
    #[test]
    fn heavy_drop_and_capacity_fail_without_losing_hand() {
        let mut arm = world(
            vec![Station::Stack; 6],
            vec![vec![1], vec![9, 9, 9, 9], vec![], vec![], vec![], vec![]],
        );
        arm.hands[0] = Some(9);
        assert_eq!(arm.act(0, 2), Err(ArmError::HeavyOnLight));
        assert_eq!(arm.hands[0], Some(9));
        arm.orientations[0] = 1;
        assert_eq!(arm.act(0, 2), Err(ArmError::OccupiedTarget));
    }
    #[test]
    fn empty_drop_at_input_is_noop_and_output_cannot_be_grabbed() {
        let mut arm = world(
            vec![
                Station::Input,
                Station::Output,
                Station::Stack,
                Station::Stack,
                Station::Stack,
                Station::Stack,
            ],
            vec![vec![]; 6],
        );
        arm.act(0, 2).unwrap();
        arm.hands[0] = Some(0);
        assert_eq!(arm.act(0, 2), Err(ArmError::InvalidDropTarget));
        arm.orientations[0] = 1;
        arm.act(0, 1).unwrap();
        assert_eq!(arm.hands[0], Some(0));
    }
    #[test]
    fn final_layout_uses_station_counts_and_faces_without_hands() {
        let initial = vec![vec![9, 1], vec![], vec![], vec![], vec![], vec![]];
        let mut arm = ArmWorld::new(
            1,
            Mode::FinalLayout,
            &vec![Station::Stack; 6],
            &initial,
            &[],
            &[2, 9, 1, 0, 0, 0, 0, 0],
        );
        assert!(arm.complete());
        assert!(arm.halt_complete());
        arm.hands[0] = Some(0);
        assert!(arm.halt_complete());
        arm.contents[0][1] ^= 4;
        assert!(!arm.halt_complete());
    }
}
