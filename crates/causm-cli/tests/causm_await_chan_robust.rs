use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn causm_robust_await_chan_sync() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      open_chan data(1)
      split main into [sender, receiver]
    }

    @sender: {
      let x = "msg"
      // Instructions advance clock (1ms each)
      // load_string(1) + move(1)
      chan_send data(x)
    }

    @receiver: {
      await_chan(data)
      let msg = chan_recv(data)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::ir::lower_program(&program);

    // Semantic analysis
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    // Execution
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    // Verify receiver got the message
    let receiver = vm
        .active_branches
        .get("receiver")
        .expect("receiver branch not found");
    let msg_reg = ir.symbols.get("msg").expect("msg symbol not found").0;

    match receiver.arena.peek(msg_reg) {
        Some(Payload::String(s)) => assert_eq!(s, "msg"),
        _ => panic!("Receiver did not receive the message payload"),
    }

    // Verify receiver clock synchronization
    // sender: load "msg"(1), move(1), slice(50), chan_send(1) = 53
    // receiver: await_chan(1) -> should align to 53ms + instruction cost?
    // Wait, Instruction::AwaitChan itself advances clock by 1 BEFORE executing the handler in execute_instruction.
    // Plus handler advances it more if it waits.

    println!(
        "Sender clock: {}",
        vm.active_branches.get("sender").unwrap().local_clock
    );
    println!("Receiver clock: {}", receiver.local_clock);

    // Receiver global time should be at least sender global time.
    let sender_gt = vm.active_branches.get("sender").unwrap().birth_global_time
        + vm.active_branches.get("sender").unwrap().local_clock;
    let receiver_gt = receiver.birth_global_time + receiver.local_clock;

    assert!(
        receiver_gt >= sender_gt,
        "Receiver global time {} should be >= sender global time {}",
        receiver_gt,
        sender_gt
    );

    Ok(())
}
