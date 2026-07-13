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

#[test]
fn causm_oop_struct_method_inheritance_and_overriding() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Parent = struct { val: int }
        routine Parent.hello(peek self) -> int taking 10ms {
            let v = self.val
            yield v
        }

        type ChildInheriting = Parent + struct {}
        type ChildOverriding = Parent + struct {}
        routine ChildOverriding.hello(peek self) -> int taking 10ms {
            let v = self.val + 100
            yield v
        }

        let c1: ChildInheriting = struct { val = 42 }
        let c2: ChildOverriding = struct { val = 42 }

        let r1 = c1.hello()
        let r2 = c2.hello()
    }
    "#;
    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let r1_reg = ir.symbols.get("r1").expect("r1 not found").0;
    let r1_val = vm.root_timeline.arena.peek(r1_reg);
    match r1_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 42),
        _ => panic!("Expected r1 to be 42, got {:?}", r1_val),
    }

    let r2_reg = ir.symbols.get("r2").expect("r2 not found").0;
    let r2_val = vm.root_timeline.arena.peek(r2_reg);
    match r2_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 142),
        _ => panic!("Expected r2 to be 142, got {:?}", r2_val),
    }
    Ok(())
}

#[test]
fn causm_oop_interface_default_methods() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        interface PlayableActor {
            routine play(peek self) -> int taking 5ms {
                let x = 777
                yield x
            }
        }
        type Robot = struct {
            id: int
        }
        let r: Robot = struct { id = 42 }
        let pa: PlayableActor = r
        let res = pa.play()
    }
    "#;
    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let res_reg = ir.symbols.get("res").expect("res not found").0;
    let res_val = vm.root_timeline.arena.peek(res_reg);
    match res_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 777),
        _ => panic!("Expected res to be 777, got {:?}", res_val),
    }
    Ok(())
}

#[test]
fn causm_oop_entropic_state_gate_constraints() -> anyhow::Result<()> {
    let source_success = r#"
    @0ms: {
        type Device = struct { id: int }
        routine Device.check(peek self) taking 10ms where self.state == Valid {
            let id = self.id
            yield id
        }
        let d: Device = struct { id = 101 }
        let ok = d.check()
    }
    "#;
    let program = parser::parse_causm(source_success)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let source_failure = r#"
    @0ms: {
        type Device = struct { id: int }
        routine Device.check(peek self) taking 10ms where self.state == Decayed {
            let id = self.id
            yield id
        }
        let d: Device = struct { id = 101 }
        let fail = d.check()
    }
    "#;
    let program_fail = parser::parse_causm(source_failure)?;
    let mut analyzer_fail = EntropicAnalyzer::new();
    let result = analyzer_fail.analyze_program(&program_fail);
    assert!(result.is_err());
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("State constraint violated") || err_msg.contains("state")
    );

    Ok(())
}

#[test]
fn causm_oop_guarded_type_assertions() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        interface Actor {
            routine act(consume self)
        }
        type Robot = struct { id: int }
        routine Robot.act(consume self) taking 1ms {}

        type Human = struct { age: int }
        routine Human.act(consume self) taking 1ms {}

        let r: Robot = struct { id = 99 }
        let a: Actor = r

        let success = 0
        let id_val = 0
        if let robot = a.(Robot) {
            success = 1
            id_val = robot.id
        } else {
            success = 2
        }

        let h: Human = struct { age = 25 }
        let a2: Actor = h
        let success2 = 0
        if let robot2 = a2.(Robot) {
            success2 = 1
        } else {
            success2 = 2
        }
    }
    "#;
    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let success_reg = ir.symbols.get("success").expect("success not found").0;
    let success_val = vm.root_timeline.arena.peek(success_reg);
    match success_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 1),
        _ => panic!("Expected success to be 1, got {:?}", success_val),
    }

    let id_reg = ir.symbols.get("id_val").expect("id_val not found").0;
    let id_val = vm.root_timeline.arena.peek(id_reg);
    match id_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 99),
        _ => panic!("Expected id_val to be 99, got {:?}", id_val),
    }

    let success2_reg = ir.symbols.get("success2").expect("success2 not found").0;
    let success2_val = vm.root_timeline.arena.peek(success2_reg);
    match success2_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 2),
        _ => panic!("Expected success2 to be 2, got {:?}", success2_val),
    }

    Ok(())
}

#[test]
fn causm_oop_if_let_reconciliation() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        interface Actor {}
        type Robot = struct { id: int }
        let r: Robot = struct { id = 42 }
        let a: Actor = r
        
        let x = struct { val = 1 }
        
        if let robot = a.(Robot) {
            let temp = x.val
        } else {
            // x remains valid
        } reconcile auto
    }
    "#;
    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    Ok(())
}
