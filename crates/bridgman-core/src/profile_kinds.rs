// Generated from profiles/thermal.yml; do not edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Mass,
    Temperature,
    TemperatureDelta,
    SpecificHeat,
    HeatCapacity,
    SpecificEnergy,
    Energy,
    Torque,
    Unitless,
    Length,
    Area,
    Time,
    ThermalConductance,
}
impl Kind {
    pub const fn dimensions(self) -> [i8; 7] {
        match self {
            Self::Mass => [1, 0, 0, 0, 0, 0, 0],
            Self::Temperature => [0, 0, 0, 0, 1, 0, 0],
            Self::TemperatureDelta => [0, 0, 0, 0, 1, 0, 0],
            Self::SpecificHeat => [0, 2, -2, 0, -1, 0, 0],
            Self::HeatCapacity => [1, 2, -2, 0, -1, 0, 0],
            Self::SpecificEnergy => [0, 2, -2, 0, 0, 0, 0],
            Self::Energy => [1, 2, -2, 0, 0, 0, 0],
            Self::Torque => [1, 2, -2, 0, 0, 0, 0],
            Self::Unitless => [0, 0, 0, 0, 0, 0, 0],
            Self::Length => [0, 1, 0, 0, 0, 0, 0],
            Self::Area => [0, 2, 0, 0, 0, 0, 0],
            Self::Time => [0, 0, 1, 0, 0, 0, 0],
            Self::ThermalConductance => [1, 2, -3, 0, -1, 0, 0],
        }
    }
}
kinds!(
    Mass,
    Temperature,
    TemperatureDelta,
    SpecificHeat,
    HeatCapacity,
    SpecificEnergy,
    Energy,
    Torque,
    Unitless,
    Length,
    Area,
    Time,
    ThermalConductance
);
linear!(
    Mass,
    TemperatureDelta,
    SpecificHeat,
    HeatCapacity,
    SpecificEnergy,
    Energy,
    Torque,
    Unitless,
    Length,
    Area,
    Time,
    ThermalConductance
);
fn kind_is_linear(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Mass
            | Kind::TemperatureDelta
            | Kind::SpecificHeat
            | Kind::HeatCapacity
            | Kind::SpecificEnergy
            | Kind::Energy
            | Kind::Torque
            | Kind::Unitless
            | Kind::Length
            | Kind::Area
            | Kind::Time
            | Kind::ThermalConductance
    )
}
