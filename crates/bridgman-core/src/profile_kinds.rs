// Generated from profiles/thermal.yml; do not edit.
kinds! {
    Mass { "M": 1 },
    Temperature { "Theta": 1 },
    TemperatureDelta { "Theta": 1 },
    SpecificHeat { "L": 2, "T": -2, "Theta": -1 },
    HeatCapacity { "M": 1, "L": 2, "T": -2, "Theta": -1 },
    SpecificEnergy { "L": 2, "T": -2 },
    Energy { "M": 1, "L": 2, "T": -2 },
    Torque { "M": 1, "L": 2, "T": -2 },
    Unitless {},
    Length { "L": 1 },
    Area { "L": 2 },
    Time { "T": 1 },
    ThermalConductance { "M": 1, "L": 2, "T": -3, "Theta": -1 },
}
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
