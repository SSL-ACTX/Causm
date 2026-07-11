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
}
