// Generated from profiles/thermal.yml; do not edit.
pub fn binary_kind(a: Kind, b: Kind, op: Op) -> Result<Kind, QuantityError> {
    use Kind::*;
    match (op, a, b) {
        (Op::Add, Temperature, TemperatureDelta) | (Op::Add, TemperatureDelta, Temperature) => {
            Ok(Temperature)
        }
        (Op::Sub, Temperature, Temperature) => Ok(TemperatureDelta),
        (Op::Sub, Temperature, TemperatureDelta) => Ok(Temperature),
        (Op::Add | Op::Sub, x, y) if x == y && kind_is_linear(x) => Ok(x),
        (Op::Mul, Mass, SpecificHeat) => Ok(HeatCapacity),
        (Op::Mul, SpecificHeat, Mass) => Ok(HeatCapacity),
        (Op::Mul, HeatCapacity, TemperatureDelta) => Ok(Energy),
        (Op::Mul, TemperatureDelta, HeatCapacity) => Ok(Energy),
        (Op::Mul, Mass, SpecificEnergy) => Ok(Energy),
        (Op::Mul, SpecificEnergy, Mass) => Ok(Energy),
        (Op::Div, Energy, Mass) => Ok(SpecificEnergy),
        (Op::Div, Energy, SpecificEnergy) => Ok(Mass),
        (Op::Mul, Length, Length) => Ok(Area),
        (Op::Div, Energy, HeatCapacity) => Ok(TemperatureDelta),
        (Op::Div, Energy, TemperatureDelta) => Ok(HeatCapacity),
        (Op::Div, HeatCapacity, Mass) => Ok(SpecificHeat),
        (Op::Div, HeatCapacity, SpecificHeat) => Ok(Mass),
        (Op::Mul, ThermalConductance, Time) => Ok(HeatCapacity),
        (Op::Mul, Time, ThermalConductance) => Ok(HeatCapacity),
        (Op::Div, HeatCapacity, Time) => Ok(ThermalConductance),
        (Op::Div, HeatCapacity, ThermalConductance) => Ok(Time),
        (Op::Mul | Op::Div, x, Unitless) if kind_is_linear(x) => Ok(x),
        (Op::Mul, Unitless, x) if kind_is_linear(x) => Ok(x),
        (Op::Div, x, y) if x == y && kind_is_linear(x) => Ok(Unitless),
        _ => Err(QuantityError::UnsupportedOperation {
            operation: Operation::Binary(op),
            left: a,
            right: Some(b),
        }),
    }
}
fn checked(kind: Kind, value: f64) -> Result<(), QuantityError> {
    if !value.is_finite() {
        return Err(QuantityError::NumericalFailure);
    }
    if kind == Kind::Temperature && value < 0.0 {
        return Err(QuantityError::BelowAbsoluteZero);
    }
    Ok(())
}
impl Quantity<Area> {
    pub fn sqrt(self) -> Result<Quantity<Length>, QuantityError> {
        if self.canonical < 0.0 {
            return Err(QuantityError::NegativeRoot);
        }
        Quantity::computed(self.canonical.sqrt())
    }
}
fn sqrt_dynamic(q: AnyQuantity) -> Result<AnyQuantity, QuantityError> {
    match q.kind {
        Kind::Area => q.try_typed::<Area>()?.sqrt().map(Into::into),
        _ => Err(QuantityError::UnsupportedOperation {
            operation: Operation::Sqrt,
            left: q.kind,
            right: None,
        }),
    }
}
operation!(Add, add, Temperature, TemperatureDelta, Temperature);
operation!(Add, add, TemperatureDelta, Temperature, Temperature);
operation!(Sub, sub, Temperature, Temperature, TemperatureDelta);
operation!(Sub, sub, Temperature, TemperatureDelta, Temperature);
operation!(Mul, mul, Mass, SpecificHeat, HeatCapacity);
operation!(Mul, mul, SpecificHeat, Mass, HeatCapacity);
operation!(Mul, mul, HeatCapacity, TemperatureDelta, Energy);
operation!(Mul, mul, TemperatureDelta, HeatCapacity, Energy);
operation!(Mul, mul, Mass, SpecificEnergy, Energy);
operation!(Mul, mul, SpecificEnergy, Mass, Energy);
operation!(Div, div, Energy, Mass, SpecificEnergy);
operation!(Div, div, Energy, SpecificEnergy, Mass);
operation!(Mul, mul, Length, Length, Area);
operation!(Div, div, Energy, HeatCapacity, TemperatureDelta);
operation!(Div, div, Energy, TemperatureDelta, HeatCapacity);
operation!(Div, div, HeatCapacity, Mass, SpecificHeat);
operation!(Div, div, HeatCapacity, SpecificHeat, Mass);
operation!(Mul, mul, ThermalConductance, Time, HeatCapacity);
operation!(Mul, mul, Time, ThermalConductance, HeatCapacity);
operation!(Div, div, HeatCapacity, Time, ThermalConductance);
operation!(Div, div, HeatCapacity, ThermalConductance, Time);
impl<K: Linear> Div for Quantity<K> {
    type Output = Result<Quantity<Unitless>, QuantityError>;
    fn div(self, b: Self) -> Self::Output {
        Quantity::computed(arithmetic(Op::Div, self.canonical, b.canonical)?)
    }
}
