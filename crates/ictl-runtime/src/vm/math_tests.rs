use crate::vm::Vm;
use ictl_core::value::Payload;
use ictl_core::BinaryOperator;
use ictl_core::UnaryOperator;

#[test]
fn test_vm_binary_ops_rem_pow() -> anyhow::Result<()> {
    let vm = Vm::new();

    // Integer Rem
    let res = vm.evaluate_binary_operation(
        Payload::Integer(10),
        Payload::Integer(3),
        &BinaryOperator::Rem,
    )?;
    assert_eq!(res, Payload::Integer(1));

    // Integer Pow
    let res = vm.evaluate_binary_operation(
        Payload::Integer(2),
        Payload::Integer(3),
        &BinaryOperator::Pow,
    )?;
    assert_eq!(res, Payload::Integer(8));

    // Float Rem
    let res = vm.evaluate_binary_operation(
        Payload::Float(10.5f64.to_bits()),
        Payload::Float(3.0f64.to_bits()),
        &BinaryOperator::Rem,
    )?;
    assert_eq!(res, Payload::Float(1.5f64.to_bits()));

    // Float Pow
    let res = vm.evaluate_binary_operation(
        Payload::Float(2.0f64.to_bits()),
        Payload::Float(0.5f64.to_bits()),
        &BinaryOperator::Pow,
    )?;
    assert_eq!(res, Payload::Float(2.0f64.sqrt().to_bits()));

    Ok(())
}

#[test]
fn test_vm_unary_ops() -> anyhow::Result<()> {
    let vm = Vm::new();

    // Neg Integer
    let res =
        vm.evaluate_unary_operation(Payload::Integer(10), &UnaryOperator::Neg)?;
    assert_eq!(res, Payload::Integer(-10));

    // Neg Float
    let res = vm.evaluate_unary_operation(
        Payload::Float(2.5f64.to_bits()),
        &UnaryOperator::Neg,
    )?;
    assert_eq!(res, Payload::Float((-2.5f64).to_bits()));

    // Not Bool
    let res =
        vm.evaluate_unary_operation(Payload::Bool(true), &UnaryOperator::Not)?;
    assert_eq!(res, Payload::Bool(false));

    let res =
        vm.evaluate_unary_operation(Payload::Bool(false), &UnaryOperator::Not)?;
    assert_eq!(res, Payload::Bool(true));

    Ok(())
}

#[test]
fn test_vm_pow_negative_exponent() -> anyhow::Result<()> {
    let vm = Vm::new();

    // 2 ^ -1 should be 0.5 (Float)
    let res = vm.evaluate_binary_operation(
        Payload::Integer(2),
        Payload::Integer(-1),
        &BinaryOperator::Pow,
    )?;
    assert_eq!(res, Payload::Float(0.5f64.to_bits()));

    Ok(())
}

#[test]
fn test_vm_intrinsics() -> anyhow::Result<()> {
    let vm = Vm::new();

    // sqrt
    let res = vm.call_intrinsic("sqrt", vec![Payload::Float(4.0f64.to_bits())])?;
    assert_eq!(res, Payload::Float(2.0f64.to_bits()));

    // sin
    let res = vm.call_intrinsic("sin", vec![Payload::Float(0.0f64.to_bits())])?;
    assert_eq!(res, Payload::Float(0.0f64.to_bits()));

    // floor
    let res = vm.call_intrinsic("floor", vec![Payload::Float(2.9f64.to_bits())])?;
    assert_eq!(res, Payload::Float(2.0f64.to_bits()));

    Ok(())
}
