//! Selected product scenes; this design catalog does not register executable scenes.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneKind {
    ExactIO,
    Baudot,
    QualityControl,
    Elevator,
    Robot,
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
            Self::Baudot => "Baudot",
            Self::QualityControl => "QualityControl",
            Self::Elevator => "Elevator",
            Self::Robot => "Robot",
        }
    }

    pub const fn descriptor(self) -> SceneDescriptor {
        let family = match self {
            Self::ExactIO | Self::Baudot | Self::QualityControl => EvaluationFamily::ExactIO,
            Self::Elevator | Self::Robot => EvaluationFamily::Environment,
        };
        let status = match self {
            Self::ExactIO => SceneStatus::Implemented,
            _ => SceneStatus::Planned,
        };
        SceneDescriptor {
            kind: self,
            family,
            status,
        }
    }
}

/// Stable product order, distinct from executable Environment capabilities.
pub const SELECTED_SCENES: [SceneKind; 5] = [
    SceneKind::ExactIO,
    SceneKind::Baudot,
    SceneKind::QualityControl,
    SceneKind::Elevator,
    SceneKind::Robot,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_preserves_selection_without_claiming_runtime_support() {
        let ids: std::collections::BTreeSet<_> =
            SELECTED_SCENES.iter().map(|scene| scene.id()).collect();
        assert_eq!(ids.len(), 5);
        assert!(!ids.contains("Terminal"));
        assert!(!ids.contains("MaintenanceRobot"));
        let descriptors = SELECTED_SCENES.map(SceneKind::descriptor);
        assert_eq!(
            descriptors
                .iter()
                .filter(|s| s.family == EvaluationFamily::ExactIO)
                .count(),
            3
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
            vec![SceneKind::ExactIO]
        );
    }
}
