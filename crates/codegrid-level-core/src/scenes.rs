//! Product catalog; implemented status tracks native API-2 registration, not WASM parity.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneKind {
    ExactIO,
    Robot,
    MechanicalArm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationFamily {
    ExactIO,
    Environment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneStatus {
    Implemented,
    Planned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneDescriptor {
    pub kind: SceneKind,
    pub family: EvaluationFamily,
    pub status: SceneStatus,
}

impl SceneKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ExactIO => "ExactIO",
            Self::Robot => "Robot",
            Self::MechanicalArm => "MechanicalArm",
        }
    }

    pub const fn descriptor(self) -> SceneDescriptor {
        let family = match self {
            Self::ExactIO => EvaluationFamily::ExactIO,
            Self::Robot | Self::MechanicalArm => EvaluationFamily::Environment,
        };
        let status = SceneStatus::Implemented;
        SceneDescriptor {
            kind: self,
            family,
            status,
        }
    }
}

/// Stable product order, distinct from executable Environment capabilities.
pub const SELECTED_SCENES: [SceneKind; 3] = [
    SceneKind::ExactIO,
    SceneKind::Robot,
    SceneKind::MechanicalArm,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_matches_native_scene_registration() {
        let ids: std::collections::BTreeSet<_> =
            SELECTED_SCENES.iter().map(|scene| scene.id()).collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains("Terminal"));
        assert!(!ids.contains("MaintenanceRobot"));
        let descriptors = SELECTED_SCENES.map(SceneKind::descriptor);
        assert_eq!(
            descriptors
                .iter()
                .filter(|s| s.family == EvaluationFamily::ExactIO)
                .count(),
            1
        );
        assert_eq!(
            descriptors
                .iter()
                .filter(|s| s.family == EvaluationFamily::Environment)
                .count(),
            2
        );
        assert_eq!(
            descriptors
                .iter()
                .filter(|s| s.status == SceneStatus::Implemented)
                .map(|s| s.kind)
                .collect::<Vec<_>>(),
            SELECTED_SCENES.to_vec()
        );
    }
}
