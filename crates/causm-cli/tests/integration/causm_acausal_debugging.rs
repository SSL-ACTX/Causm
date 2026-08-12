#[cfg(test)]
mod tests {
    use causm_core::value::Payload;
    use causm_frontend::parser;
    use causm_runtime::vm::state::Vm;

    #[test]
    fn test_step_back() -> anyhow::Result<()> {
        let code = r#"
@main: {
  let x = 100
  let x = 200
  let x = 300
}
"#;
        let program = parser::parse_causm(code)?;
        let ir = causm_frontend::lower::lower_program(&program);
        let mut vm = Vm::new();
        vm.debug_mode = true;

        println!("Symbols: {:?}", ir.symbols);
        println!("Instructions: {:?}", ir.blocks[0].instructions);

        // Set instructions for main branch
        {
            let main = vm.get_branch_mut("main")?;
            main.instructions = ir.blocks[0].instructions.clone();
        }

        let x_reg = ir.symbols.get("x").expect("x register not found").0;
        println!("x_reg: {}", x_reg);

        // Execute 'let x = 100' (LoadInt + Move)
        println!("Executing 1st let");
        vm.execute_instruction("main")?;
        vm.execute_instruction("main")?;
        let val1 = vm.peek_reg("main", x_reg)?;
        assert_eq!(val1, Payload::Integer(100));

        // Execute 'let x = 200' (LoadInt + Move)
        println!("Executing 2nd let");
        vm.execute_instruction("main")?;
        vm.execute_instruction("main")?;
        let val2 = vm.peek_reg("main", x_reg)?;
        assert_eq!(val2, Payload::Integer(200));

        // Step back 1: should restore state before 'let x = 200' (which is x=100)
        // Step back twice to restore state before the assignment (Move and LoadInt instructions).
        println!("Stepping back twice to x=100");
        vm.step_back("main")?; // before Move
        vm.step_back("main")?; // before LoadInt
        let val_back1 = vm.peek_reg("main", x_reg)?;
        assert_eq!(val_back1, Payload::Integer(100));

        // Step back 2: should restore state before 'let x = 100' (which is x undefined)
        println!("Stepping back twice more to x undefined");
        vm.step_back("main")?;
        vm.step_back("main")?;
        let x_state = vm.peek_state("main", x_reg)?;
        assert!(matches!(
            x_state,
            causm_core::value::EntropicState::Consumed
        ));

        Ok(())
    }

    #[test]
    fn test_decay_tracking() -> anyhow::Result<()> {
        let code = r#"
@main: {
  let p = struct { x: 1, y: 2 }
  let a = p.x
}
"#;
        let program = parser::parse_causm(code)?;
        let ir = causm_frontend::lower::lower_program(&program);
        let mut vm = Vm::new();
        vm.execute_program(&ir)?;

        let decay_events: Vec<_> = vm
            .causal_history
            .iter()
            .filter(|e| {
                matches!(e, causm_runtime::vm::state::CausalEvent::Decay { .. })
            })
            .collect();
        assert!(!decay_events.is_empty());
        if let causm_runtime::vm::state::CausalEvent::Decay { field, .. } =
            &decay_events[0]
        {
            assert_eq!(field, "x");
        } else {
            panic!("Expected decay event");
        }

        Ok(())
    }
}
