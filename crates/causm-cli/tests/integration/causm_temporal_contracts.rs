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

    #[test]
    fn test_temporal_method_contract_budget_exceeded() -> anyhow::Result<()> {
        let code = r#"
@main: {
  type SensorStream = struct { rate: int }

  routine SensorStream.sample(peek self) -> int (taking 2ms) {
    let x = self.rate + 1
    let y = x + 1
    let z = y + 1
    yield z
  }
}
"#;
        let res = run_causm(code);
        match res {
            Err(e) => {
                let err = e.to_string();
                assert!(err.contains("Routine temporal contract violated"));
            }
            Ok(_) => {
                panic!("Should have failed analysis due to WCET contract violation")
            }
        }
        Ok(())
    }

    #[test]
    fn test_syntax_duration_wildcard_inferred_and_profile_guided(
    ) -> anyhow::Result<()> {
        let code = r#"
@main: {
  routine process_inferred(peek data: string) -> bool taking _ {
    yield true
  }

  routine calculate_tuned(samples: array) -> array taking ? {
    yield samples
  }

  interface FftProcessor {
    routine execute(samples: array) -> array taking ?
    routine fast_step() -> bool taking _
  }
}
"#;
        let program = parser::parse_causm(code)?;
        assert_eq!(program.timelines.len(), 1);
        let statements = &program.timelines[0].statements;

        // Assert process_inferred AST has taking_ms == None (inferred contract)
        let found_inferred = statements.iter().any(|s| {
            if let causm_core::Statement::RoutineDef {
                name, taking_ms, ..
            } = &s.stmt
            {
                name == "process_inferred" && taking_ms.is_none()
            } else {
                false
            }
        });
        assert!(found_inferred, "Routine process_inferred with taking _ should be parsed with taking_ms = None");

        // Assert calculate_tuned AST has taking_ms == None (profile-guided contract)
        let found_tuned = statements.iter().any(|s| {
            if let causm_core::Statement::RoutineDef {
                name, taking_ms, ..
            } = &s.stmt
            {
                name == "calculate_tuned" && taking_ms.is_none()
            } else {
                false
            }
        });
        assert!(found_tuned, "Routine calculate_tuned with taking ? should be parsed with taking_ms = None");

        // Assert interface methods parsed properly with taking_ms == None
        let found_interface = statements.iter().any(|s| {
            if let causm_core::Statement::InterfaceDecl { name, methods, .. } =
                &s.stmt
            {
                if name == "FftProcessor" && methods.len() == 2 {
                    methods[0].name == "execute"
                        && methods[0].taking_ms.is_none()
                        && methods[1].name == "fast_step"
                        && methods[1].taking_ms.is_none()
                } else {
                    false
                }
            } else {
                false
            }
        });
        assert!(found_interface, "Interface methods with taking ? and taking _ should be parsed with taking_ms = None");

        Ok(())
    }
}
