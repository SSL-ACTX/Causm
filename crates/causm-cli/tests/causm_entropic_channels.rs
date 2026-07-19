#[cfg(test)]
mod tests {
    use causm_analysis::analyzer::EntropicAnalyzer;
    use causm_frontend::parser;
    use causm_runtime::vm::state::Vm;

    fn run_causm(code: &str) -> anyhow::Result<Vm> {
        let program = parser::parse_causm(code)?;

        let mut analyzer = EntropicAnalyzer::new();
        analyzer
            .analyze_program(&program)
            .map_err(|e| anyhow::anyhow!(e))?;

        let ir_program = causm_frontend::lower::lower_program(&program);
        let mut vm = Vm::new();
        vm.execute_program(&ir_program)?;
        Ok(vm)
    }

    #[test]
    fn test_entropic_channel_sync() -> anyhow::Result<()> {
        let code = r#"
@main: {
  open_chan data(1)
  split main into [sender, receiver]
}

@sender: {
  // Advance sender clock
  let dummy = 1
  let dummy2 = 2
  let x = 42
  chan_send data(x)
}

@receiver: {
  await_chan(data)
  let val = chan_recv(data)
  assert_time(elapsed >= 3ms)
}
"#;
        let vm = run_causm(code)?;
        let receiver = vm.active_branches.get("receiver").unwrap();
        // sender: 1 (start) + 1 (let dummy) + 1 (let dummy2) + 1 (let x) + 1 (chan_send) = 5
        // receiver: 1 (start) + 1 (await_chan) -> waits for sender at 5ms -> 5 + 1 (let val) + 1 (assert_time) = 7
        println!("Receiver local clock: {}", receiver.local_clock);
        assert!(receiver.local_clock >= 5);
        Ok(())
    }

    #[test]
    fn test_entropic_channel_manifest_violation() -> anyhow::Result<()> {
        let code = r#"
@main: {
  open_chan data(1)
  isolate Worker {
    // Missing Chan.Outbound
    let x = 10
    chan_send data(x)
  }
}
"#;
        let res = run_causm(code);
        match res {
            Err(e) => {
                let err = e.to_string();
                println!("Error: {}", err);
                assert!(err.contains("Capability violation"));
                assert!(err.contains("Chan.Outbound(id=data)"));
            }
            Ok(_) => panic!("Should have failed analysis"),
        }
        Ok(())
    }

    #[test]
    fn test_entropic_channel_manifest_success() -> anyhow::Result<()> {
        let code = r#"
@main: {
  open_chan data(1)
  isolate Worker {
    require Chan.Outbound(id="data")
    let x = 10
    chan_send data(x)
  }
}
"#;
        let res = run_causm(code);
        assert!(res.is_ok());
        Ok(())
    }

    #[test]
    fn test_entropic_channel_message_lease_decay_and_handler() -> anyhow::Result<()>
    {
        let code = r#"
@main: {
  type Packet = struct { seq: int }
  let decay_triggered = false
  decay_handler for Packet {
    decay_triggered = true
  }
  open_chan data(1) decay_after 5ms
  split main into [sender, receiver]
}

@sender: {
  let p: Packet = struct { seq = 1 }
  chan_send data(p)
}

@receiver: {
  let d1 = 1
  let d2 = 2
  let d3 = 3
  let d4 = 4
  let d5 = 5
  let d6 = 6
  let d7 = 7
  await_chan(data)
  let p_received = chan_recv(data)
}
"#;
        let vm = run_causm(code)?;
        let receiver = vm.active_branches.get("receiver").unwrap();
        let decay_triggered_reg = vm
            .symbols
            .get("decay_triggered")
            .expect("decay_triggered symbol not found")
            .0;
        let val = receiver.arena.peek(decay_triggered_reg);
        match val {
            Some(causm_core::value::Payload::Bool(b)) => {
                assert!(b, "Expected decay_triggered to be true")
            }
            _ => panic!("Expected decay_triggered to be true bool, got {:?}", val),
        }
        Ok(())
    }
}
