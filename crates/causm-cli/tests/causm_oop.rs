use causm_analysis::analyzer::{EntropicAnalyzer, SemanticErrorKind};
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn causm_oop_basic_method_call() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct { score: int }

        routine Player.get_score(peek self) -> int (taking 5ms) {
            let s = self.score
            yield s
        }

        let p: Player = struct { score = 42 }
        let s = p.get_score()
        let _s = s
        let _p = p
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_oop_method_consume() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct { score: int }

        routine Player.retire(consume self) (taking 3ms) {
            let _self = self
        }

        let p: Player = struct { score = 42 }
        let _r = p.retire()
        let _p = p
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::UseAfterConsume(name) => assert_eq!(name, "p"),
        _ => panic!("Expected UseAfterConsume, got {:?}", err.kind),
    }

    Ok(())
}

#[test]
fn causm_oop_method_type_mismatch() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct { score: int }

        routine Player.get_score(peek self) -> int (taking 5ms) {
            let s = self.score
            yield s
        }

        let p: Player = struct { score = 42 }
        let s = p.get_score(10)
        let _s = s
        let _p = p
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::ArgumentCountMismatch(_) => {}
        _ => panic!("Expected ArgumentCountMismatch, got {:?}", err.kind),
    }

    Ok(())
}

#[test]
fn causm_oop_constructor_call() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct { score: int }

        routine Player.new(clone score: int) -> Player (taking 4ms) {
            let p: Player = struct { score = score }
            yield p
        }

        let p: Player = call Player.new(42)
        let _p = p
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_oop_method_chaining() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct { score: int }

        routine Player.add_score(consume self, clone amount: int) -> Player (taking 10ms) {
            let new_score = self.score + amount
            let p2: Player = struct { score = new_score }
            yield p2
        }

        let p: Player = struct { score = 42 }
        let p2: Player = p.add_score(5).add_score(10)
        let s = p2.score
        let _s = s
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_oop_associated_constants() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Config = struct {
            const MAX_CONNECTIONS: int = 100,
            port: int
        }

        let limit = Config.MAX_CONNECTIONS
        let _limit = limit
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_oop_default_field_values() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Config = struct {
            const DEFAULT_PORT: int = 8080,
            port: int = 8080,
            host: string = "localhost"
        }

        let c: Config = struct {}
        let p = c.port
        let h = c.host
        let _p = p
        let _h = h
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_oop_encapsulation_success() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Account = struct {
            _balance: int = 100
        }

        routine Account.get_balance(peek self) -> int (taking 4ms) {
            let b = self._balance
            yield b
        }

        let a: Account = struct {}
        let b = a.get_balance()
        let _b = b
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_oop_encapsulation_failure_field() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Account = struct {
            _balance: int = 100
        }

        let a: Account = struct {}
        let b = a._balance
        let _b = b
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.kind.to_string().contains("private"));

    Ok(())
}

#[test]
fn causm_oop_encapsulation_failure_method() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Account = struct {
            balance: int = 100
        }

        routine Account._secret(peek self) -> int (taking 4ms) {
            let b = self.balance
            yield b
        }

        let a: Account = struct {}
        let b = a._secret()
        let _b = b
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.kind.to_string().contains("private"));

    Ok(())
}

#[test]
fn causm_oop_interfaces() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        interface Actor {
            routine act(consume self) -> int (taking 10ms)
        }

        type Robot = struct {
            id: int
        }

        routine Robot.act(consume self) -> int (taking 5ms) {
            let id = self.id
            yield id
        }

        let r: Robot = struct { id = 42 }
        let a: Actor = r
        let id = a.act()
        let _id = id
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_oop_composition() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct {
            score: int = 10
        }

        type SpecialPlayer = Player + struct {
            bonus: int = 5
        }

        let p: SpecialPlayer = struct {}
        let s = p.score
        let b = p.bonus
        let _s = s
        let _b = b
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_oop_downcast_type_assertion() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        interface Actor {
            routine act(consume self)
        }
        type Robot = struct {
            name: string
        }
        routine Robot.act(consume self) taking 1ms {
            // do nothing
        }
        let r: Robot = struct { name = "Terminator" }
        let a: Actor = r
        let concrete = a.(Robot)
        let name = concrete.name
    }
    "#;
    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let name_reg = ir.symbols.get("name").expect("name not found").0;
    let name_val = vm.root_timeline.arena.peek(name_reg);
    match name_val {
        Some(causm_core::value::Payload::String(s)) => assert_eq!(s, "Terminator"),
        _ => panic!("Expected concrete.name to be Terminator"),
    }
    Ok(())
}

#[test]
fn causm_oop_interface_composition() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        interface Actor {
            routine act(consume self)
        }
        interface PlayableActor = Actor + interface {
            routine play(consume self)
        }
        type Robot = struct {
            name: string
        }
        routine Robot.act(consume self) taking 1ms {
            // do nothing
        }
        routine Robot.play(consume self) taking 1ms {
            // do nothing
        }
        let r: Robot = struct { name = "Terminator" }
        let pa: PlayableActor = r
    }
    "#;
    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    Ok(())
}

#[test]
fn causm_oop_dynamic_budget_enforcement() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        isolate worker_isolate {
            enable cpu(100ms)
            enable memory(50KB)
            require System.NetworkFetch

            interface Worker {
                routine work(consume self) taking 10ms
            }
            type SlowWorker = struct {
                name: string
            }
            routine SlowWorker.work(consume self) taking 10ms {
                let dataset = defer System.NetworkFetch(url="api.data", latency="15") deadline 50ms
                await(dataset)
            }
            let w: SlowWorker = struct { name = "lazy" }
            let worker: Worker = w
            let _res = worker.work()
        }
    }
    "#;
    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut vm = Vm::new();
    vm.register_capability("System.NetworkFetch", |_params| Ok(()));
    let res = vm.execute_program(&ir);
    assert!(res.is_err());
    let err_msg = format!("{:?}", res.err().unwrap());
    assert!(err_msg.contains("temporal contract violated"));
    Ok(())
}

#[test]
fn causm_oop_multilevel_struct_composition() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type A = struct {
            const x: int = 100,
            y: int = 200
        }
        type B = A + struct {
            const z: int = 300,
            w: int = 400
        }
        type C = B + struct {
            v: int = 500
        }
        let c: C = struct {}
        let val_x = C.x
        let val_y = c.y
        let val_z = C.z
        let val_w = c.w
        let val_v = c.v
    }
    "#;
    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let y_reg = ir.symbols.get("val_y").expect("val_y not found").0;
    let y_val = vm.root_timeline.arena.peek(y_reg);
    match y_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 200),
        _ => panic!("Expected y=200"),
    }

    let w_reg = ir.symbols.get("val_w").expect("val_w not found").0;
    let w_val = vm.root_timeline.arena.peek(w_reg);
    match w_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 400),
        _ => panic!("Expected w=400"),
    }

    let v_reg = ir.symbols.get("val_v").expect("val_v not found").0;
    let v_val = vm.root_timeline.arena.peek(v_reg);
    match v_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 500),
        _ => panic!("Expected v=500"),
    }

    Ok(())
}
