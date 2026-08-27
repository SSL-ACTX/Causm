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

    #[test]
    fn test_temporal_static_inference_taking_wildcard_execution(
    ) -> anyhow::Result<()> {
        let code = r#"
@main: {
  routine compute_sum(peek a: int, peek b: int) -> int taking _ {
    let sum = a + b
    yield sum
  }

  let x = 10
  let y = 20
  let res = compute_sum(x, y)
}
"#;
        let program = parser::parse_causm(code)?;
        let mut analyzer = EntropicAnalyzer::new();
        analyzer.analyze_program(&program)?;

        let ir = causm_frontend::lower::lower_program(&program);
        let mut vm = Vm::new();
        vm.execute_program(&ir)?;

        let res_reg = ir.symbols.get("res").expect("res symbol not found").0;
        let val = vm.root_timeline.arena.peek(res_reg);
        match val {
            Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 30),
            _ => panic!("Expected compute_sum to produce 30, got {:?}", val),
        }
        Ok(())
    }

    #[test]
    fn test_temporal_interface_contract_subtyping_satisfaction_and_violation(
    ) -> anyhow::Result<()> {
        // 1. Success scenario: concrete routine inferred cost <= interface budget
        let code_ok = r#"
@0ms: {
  interface FastWorker {
    routine work(peek self) -> int (taking 10ms)
  }

  type Task = struct { val: int }

  routine Task.work(peek self) -> int taking _ {
    let v = self.val
    yield v
  }

  let t: Task = struct { val = 42 }
  let w: FastWorker = t
  let res = w.work()
}
"#;
        let program = parser::parse_causm(code_ok)?;
        let mut analyzer = EntropicAnalyzer::new();
        analyzer.analyze_program(&program)?;

        let ir = causm_frontend::lower::lower_program(&program);
        let mut vm = Vm::new();
        vm.execute_program(&ir)?;

        let res_reg = ir.symbols.get("res").expect("res symbol not found").0;
        let res_val = vm.root_timeline.arena.peek(res_reg);
        match res_val {
            Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 42),
            _ => panic!("Expected work to produce 42, got {:?}", res_val),
        }

        // 2. Failure scenario: concrete routine explicit or inferred cost exceeds interface budget
        let code_fail = r#"
@0ms: {
  interface TightWorker {
    routine work(peek self) -> int (taking 1ms)
  }

  type HeavyTask = struct { val: int }

  routine HeavyTask.work(peek self) -> int taking 5ms {
    let a = self.val + 1
    let b = a + 1
    yield b
  }

  let t: HeavyTask = struct { val = 1 }
  let w: TightWorker = t
}
"#;
        let res_fail = run_causm(code_fail);
        assert!(res_fail.is_err(), "Expected type compatibility failure when routine budget exceeds interface contract");
        Ok(())
    }

    #[test]
    fn test_devtools_profiler_and_tuner_rewriter() -> anyhow::Result<()> {
        let code = r#"
@0ms: {
  routine process_sample(peek s: int) -> int taking ? {
    let a = s + 10
    yield a
  }
  let x = 5
}
"#;
        let program = parser::parse_causm(code)?;
        let ir = causm_frontend::lower::lower_program(&program);
        let mut vm = Vm::new();
        vm.execute_program(&ir)?;

        // 1. Validate real memory and clock profiling
        let report =
            causm_devtools::profiler::timeline::TimelineProfileReport::profile_vm(
                &vm,
            );
        assert!(
            report.memory.capacity_bytes > 0,
            "Profiler should record arena memory capacity"
        );
        assert_eq!(report.clock.global_clock_ms, 0);

        // 2. Validate AST contract rewriter
        let patched = causm_devtools::tuner::rewriter::patch_routine_contract(
            code,
            "process_sample",
            46,
        );
        assert!(patched
            .contains("routine process_sample(peek s: int) -> int taking 46ms"));
        assert!(!patched.contains("taking ?"));

        // 3. Validate statistical WCET calculation
        let p99 = causm_devtools::tuner::statistics::calculate_p99_wcet(
            &[10, 12, 14, 15, 18],
            10.0,
        );
        assert!(p99 >= 18);

        Ok(())
    }

    #[test]
    fn test_ffi_array_buffer_pointer_mutation() -> anyhow::Result<()> {
        let code = r#"
foreign "libc.so.6" abi("C") {
    pub routine memset(peek s: array, c: i32, n: u64) -> i64 taking 1ms
}

@main: {
    let mut buf = [0, 0, 0, 0]
    let res = memset(buf, 65, 4)
    let b0 = buf[0]
    let b3 = buf[3]
}
"#;
        let vm = run_causm(code)?;
        let b0_val = vm.root_timeline.arena.peek(vm.symbols.get("b0").unwrap().0);
        let b3_val = vm.root_timeline.arena.peek(vm.symbols.get("b3").unwrap().0);
        assert_eq!(b0_val, Some(causm_core::value::Payload::Integer(65)));
        assert_eq!(b3_val, Some(causm_core::value::Payload::Integer(65)));
        Ok(())
    }
}
