use causm_frontend::ir::{lower_program, Instruction, Reg};
use causm_frontend::parser::parse_causm;
use causm_jit::hw_timing;
use causm_runtime::vm::Vm;

#[test]
fn test_hardware_proof_isochronous() {
    // We'll define two routines that do almost nothing but take a specific number of cycles.
    let code = r#"
        @0ms: {
            routine contract_50k() -> int taking 50000 cycles {
                let res = 0
                yield res
            }
            routine contract_100k() -> int taking 100000 cycles {
                let res = 0
                yield res
            }
        }
    "#;

    let program = parse_causm(code).unwrap();
    let ir = lower_program(&program);
    let mut vm = Vm::new();

    // Call 50k contract (Warmup)
    let mut call_50k = ir.clone();
    call_50k.blocks[0].instructions.push(Instruction::Call {
        routine: "contract_50k".to_string(),
        args: vec![],
        dest: Reg(100),
    });
    vm.execute_program(&call_50k).unwrap();

    // Call 100k contract (Warmup)
    let mut call_100k = ir.clone();
    call_100k.blocks[0].instructions.push(Instruction::Call {
        routine: "contract_100k".to_string(),
        args: vec![],
        dest: Reg(101),
    });
    vm.execute_program(&call_100k).unwrap();

    // MEASURE 50k
    let t1 = hw_timing::read_tsc();
    vm.execute_program(&call_50k).unwrap();
    let t2 = hw_timing::read_tsc();
    let delta_50k = t2 - t1;

    // MEASURE 100k
    let t3 = hw_timing::read_tsc();
    vm.execute_program(&call_100k).unwrap();
    let t4 = hw_timing::read_tsc();
    let delta_100k = t4 - t3;

    println!("Hardware Proof (TSC Deltas):");
    println!("50k Contract:  {} cycles", delta_50k);
    println!("100k Contract: {} cycles", delta_100k);

    let diff = delta_100k as i64 - delta_50k as i64;
    println!("Observed Difference: {} cycles (Target: 50000)", diff);

    // The difference between the two calls should be VERY close to 50,000 cycles.
    // We allow a small margin for VM overhead (dispatching the call).
    assert!(
        (diff - 50000).abs() < 2000,
        "Hardware clock deviation too high! Padding failed."
    );
}
