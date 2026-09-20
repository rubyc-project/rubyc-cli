# Classes, inheritance and access modifiers

## Classes and objects

```rubyc
namespace game {
    class player {
        public int hp = 100;

        public int Damage(int amount) {
            this.hp = this.hp - amount;
            return this.hp;
        }
    }

    class app {
        int main() {
            player p = new player();
            printf("{0}", p.Damage(30));   // → 70
        }
    }
}
```

- `new ClassName()` allocates on the runtime heap, stores the class vtable,
  and runs field initializers in declaration order.
- Methods receive the instance as an implicit receiver; inside the class,
  bare names (`hp`) and `this.hp` are equivalent.
- Calls dispatch virtually: the receiver's runtime vtable picks the nearest
  override in the hierarchy.

## Inheritance

```rubyc
class enemy {
    public int hp = 10;
    public void Hit() { printf("enemy hit"); }
}

class boss : enemy {
    public override-shape int Hit() { printf("boss hit"); }
}
```

A class lists its bases after `:`. Multiple inheritance of classes is
supported; diamond conflicts resolve with explicit parent qualification.

## Access modifiers

| Modifier | Visible from |
|---|---|
| `public` | anywhere |
| `private` (or no modifier) | declaring class only |
| `protected` | declaring class + derived classes |

Violations fail compilation with stable codes:

```text
error[E0017]: 'p.hp' is private and is only accessible within 'p'
error[E0018]: 'base.Hit' is protected and is only accessible within 'base' or a derived class
```

C# default applies: **no modifier means private**.

## Interfaces

Interfaces declare method signatures a class must implement:

```rubyc
interface Closeable {
    void Close();
}
class File : Closeable {
    public void Close() { /* ... */ }
}
```

Conformance is verified during semantic analysis; missing methods fail the
build before codegen.
