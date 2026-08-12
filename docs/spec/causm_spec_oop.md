# Specification: Object-Oriented Programming (OOP) in Causm

## 1. Core Principles
Causm provides a lightweight, entropic-aware Object-Oriented Programming (OOP) model. Unlike traditional languages that bundle state mutation directly, Causm enforces **Entropic Safety** and **Temporal Budgets** on method invocations and instantiation.

## 2. Struct Declarations

Structs represent structured records. A struct type is declared using the `type` keyword.

### 2.1 Default Field Values
Fields can specify optional default values to simplify initialization.
```causm
type Player = struct {
    score: int = 0,
    level: int = 1,
    name: string = "Anonymous"
}
```
If an instantiation omits fields that have default values, the compiler automatically populates them at construction time.

### 2.2 Associated Constants
Associated constants represent static, type-scoped compile-time constants. They are defined using the `const` keyword inside the struct definition.
```causm
type Config = struct {
    const MAX_SCORE: int = 9999,
    port: int = 8080
}
```
Associated constants do not occupy instance memory and are accessed via dot-notation directly on the type name:
```causm
let limit = Config.MAX_SCORE
```

## 3. Methods

Methods are routines associated with a struct type. They are defined using dot-notation: `TypeName.MethodName`.

### 3.1 Receiver Parameter
The first parameter of a method must be `self`, which represents the instance receiver. The type of `self` is automatically inferred to be the struct type.
The receiver parameter requires a parameter mode (`consume`, `lease`, `peek`, or `clone`):
*   `consume self`: Consumes the instance, moving ownership into the method. The caller cannot use the instance after this call.
*   `lease self`: Borrows the instance under a temporal lease for the duration of the method's WCET budget. Equivalent to a time-bounded read-write borrow — the instance is returned to the caller's scope automatically when the lease expires.
*   `peek self`: Grants read-only access (borrow) without consuming or copying.
*   `clone self`: Automatically clones the instance.

```causm
routine Player.get_score(peek self) -> int (taking 2ms) {
    let s = self.score
    yield s
}
```

### 3.2 Method Chaining
Methods that consume `self` and return a new instance of the type can be chained together in a fluent API style.
```causm
routine Player.add_score(consume self, clone amount: int) -> Player (taking 10ms) {
    let new_score = self.score + amount
    let p2: Player = struct { score = new_score }
    yield p2
}

let p: Player = struct { score = 42 }
let p2: Player = p.add_score(5).add_score(10)
```

## 4. Static Methods & Constructors

Routines declared on a type that do not have `self` as their first parameter act as **Static Methods** or **Constructors**.
```causm
routine Player.new(clone score: int) -> Player (taking 4ms) {
    let p: Player = struct { score = score }
    yield p
}
```
Static methods are invoked using the `call` keyword followed by the dot-separated routine name:
```causm
let p: Player = call Player.new(42)
```

## 5. Struct Composition & Inheritance

Causm supports struct subtyping and composition through the `+` operator.

### 5.1 Inheritance & Field Composition
A struct type can extend a base struct type:
```causm
type Actor = struct {
    name: string
}

type Robot = Actor + struct decay_after 100ms {
    model: string
}
```
`Robot` contains both the `name` field from `Actor` and its own `model` field.

### 5.2 Dynamic Method Inheritance & Overriding
Derived types automatically inherit the methods defined on their base types. If the derived type defines a method with the same name, it overrides the base type's implementation:
```causm
// Inherited by Robot if not overridden
routine Actor.introduce(peek self) -> int (taking 10ms) {
    print("Hello, I am Actor: " + self.name)
    yield 0
}

// Overridden by Robot
routine Robot.introduce(peek self) -> int (taking 15ms) {
    print("Robot Model " + self.model + " reporting.")
    yield 0
}
```

## 5a. Generic Structs & Monomorphized Dispatch

Structs can declare generic type parameters with entropic bounds. The compiler monomorphizes each instantiation into specialized IR at compile time.

```causm
type Container<T: Consumable> = struct {
    value: T
}

routine Container<T>.take_inner(consume self) -> T taking 10ms {
    let inner = self.value
    yield inner
}

let c: Container<int>   = struct { value = 42 }
let v: int = c.take_inner() // resolved to monomorphized Container<int>.take_inner
```

### Supported Type Bounds
| Bound | Meaning |
|-------|---------|
| `Consumable` | The type parameter can be consumed (moved) |
| `Leasable` | The type parameter supports temporal lease borrowing |

See [Type System spec](causm_spec_types.md#4a-generic-type-parameters) for full bounds reference.

## 6. Interfaces & Dynamic Dispatch

Interfaces define a set of methods that concrete types can implement.

### 6.1 Interface Declarations
```causm
interface Worker {
    routine work(consume self) -> int taking 20ms
}
```

### 6.2 Default Method Implementations
Interfaces can provide default method implementations:
```causm
interface PlayableWorker = Worker + interface {
    routine play(peek self) -> int taking 20ms {
        let bonus = 100
        yield bonus
    }
}
```

### 6.3 Associated Lifecycle Types in Interfaces
Interfaces can declare associated entropic types and decay constraints, allowing generic contract specifications:
```causm
interface Streamable {
    type PayloadType: Consumable
    decay_after 500ms

    routine next(peek self) -> PayloadType taking 10ms
}
```
- `type PayloadType: Consumable` declares an associated type with an entropic bound.
- `decay_after 500ms` constrains implementing structs to an entropic lifetime.
- Implementing structs must concretely specify `PayloadType` when satisfying the interface.

### 6.4 Interface Subtyping & Implicit Implementation
Causm uses structural subtyping: any struct that defines all methods required by an interface implicitly implements that interface.
```causm
let w: Worker = r // Struct subtyping assignment
let bonus = w.play() // Dynamic dispatch (resolves to default implementation)
```

## 7. Guarded Type Assertions (Downcasting)

Interface variables can be downcasted back to concrete structs using guarded `if let` blocks:
```causm
if let robot = w.(Robot) {
    inspect r_temp = robot {
        let model = r_temp.model
        print("Model: " + model)
    }
    robot.work()
}
```
