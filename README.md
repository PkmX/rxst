# Rxst

[Documentation](https://pkmx.github.io/rxst/rxst/index.html)

`Rxst` is a reactive programming library for Rust inspired by [Scala.Rx](https://github.com/lihaoyi/scala.rx).

## Example

```rust
#[macro_use]
extern crate rxst;
use rxst::*;

fn main() {
    // Declare `x` as a reactive variable of type `i32`.
    let x = RxCell::<i32>::new(0);

    // `y` is reactive node that depends on `x`, and its value is always `x` plus 1.
    let y = rx!(x, |x| x + 1);

    // `xy` is a reactive node that depends on both `x` and `y`, whose value is
    // the product of both.
    let xy = rx!(x, y, |x, y| x * y);

    // Set the value of `x` to 2 and trigger a re-calculation of `y` and `xy`.
    x.set(2);
    assert_eq!(x.get(), 2);
    assert_eq!(y.get(), 3);
    assert_eq!(xy.get(), 6);

    // Observe changes to reactive variables.
    xy.observe(|n| { println!("xy is {}", n); });
    x.set(3); // prints "xy is 12"
}
```

## Reactive Variables

**Reactive Variables** (`Rx`) are values that can change over time. `Rxst` provides two kinds of Rx:

* `RxCell` is a reactive cell that can be set. They act as the origin of changes in a reactive system from external sources.
* `RxNode` represents a reactive computation that depends on other Rxs. It is re-calculated when any of its dependencies are updated.

## Observers

Each Rx may also be listened for changes with `observe`. This is useful for performing side effects in response to changes of the reactive value. Note that the registered closure must outlive the Rx.

While it is not required or enforced, it is recommended that you only perform side effects in observers while the computation in `RxNode` remains side-effect free.

## Dataflow Graph

Rxs and their dependencies form a *directed acyclic graph* (DAG) where the nodes of the DAG are Rxs and the edges are the dataflow between Rxs.

To avoid inconsistent states (i.e. *glitches*), Rxs in a dataflow graph are updated in topological order:

```rust
let a = RxCell::new(0);
let b = rx!(a, |a| a + 1);
let c = rx!(a, |a| a + 1);

// `d` is always updated after `b` and `c`. A naive implementation may
// have `b` trigger `d` before `c` is updated and thus violate the
// invariant assumed by `d`.
let d = rx!(b, c, |b, c| assert_eq!(b, c));

a.set(42);
```

## Lifetime

Rx in Rxst are `Box`ed as the implementation uses raw pointers under the hood to avoid
reference counting, and that requires the address of each Rx to be stable. Rxst also annotates
each Rx so that a node cannot outlive its dependencies, and that each node is properly
unregistered from its sources when dropped. This also ensures that you can't
construct a cyclic dependency between Rxs.

```rust
let a = RxCell::new(0);
{
    let b = rx!(a, |a| a + 1);
    b.observe(|b| println!("b is {}", b));
    a.set(1);
    a.set(2);
} // b is dropped here

a.set(3); // prints nothing
```

# Limitations

* The value in Rx is currently limited to `T: Copy + 'static`.

* Rxs cannot be empty and they also do not *terminate*. You can partially model empty Rxs with `Option<T>`.

* `RxNode` can only depend on at most 4 Rxs as arguments, although this is simply a limitation of the implementation and can be easily increased.

* Observers cannot be unregistered from Rxs yet.

* `Rxst` currently only offer the most basic combinators for Rxs. More shall follow in the near future (merging, filtering, etc).

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
