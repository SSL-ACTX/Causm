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
    fn test_paced_iterable_violation() -> anyhow::Result<()> {
        let code = r#"
@main: {
  routine process_data(consume data: PacedIterable<int, 2ms>) taking 100ms {
    for item consume data {
      let x = item + 1
      let y = x + 1
    }
  }
}
"#;
        let res = run_causm(code);
        match res {
            Err(e) => {
                let err = e.to_string();
                println!("Error: {}", err);
                assert!(err.contains("exceeds PacedIterable contract"));
            }
            Ok(_) => panic!("Should have failed analysis"),
        }
        Ok(())
    }

    #[test]
    fn test_constant_access_violation() -> anyhow::Result<()> {
        let code = r#"
@main: {
  type Point = struct { x: int, y: int }

  routine calculate(clone p: ConstantAccess<Point, 10ms>) taking 15ms {
    let a = p.x
    let b = p.y
  }
}
"#;
        let res = run_causm(code);
        match res {
            Err(e) => {
                let err = e.to_string();
                println!("Error: {}", err);
                assert!(err.contains("Routine temporal contract violated"));
            }
            Ok(_) => panic!("Should have failed analysis"),
        }
        Ok(())
    }
}
