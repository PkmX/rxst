//! # Rxst
//!
//! [Documentation](https://pkmx.github.io/rxst/rxst/index.html)
//!
//! `Rxst` is a reactive programming library for Rust inspired by [Scala.Rx](https://github.com/lihaoyi/scala.rx).
//!
//! ## Example
//!
//! ```
//! #[macro_use]
//! extern crate rxst;
//! use rxst::*;
//!
//! fn main() {
//!     // Declare `x` as a reactive variable of type `i32`.
//!     let x = RxCell::<i32>::new(0);
//!
//!     // `y` is reactive node that depends on `x`, and its value is always `x` plus 1.
//!     let y = rx!(x, |x| x + 1);
//!
//!     // `xy` is a reactive node that depends on both `x` and `y`, whose value is
//!     // the product of both.
//!     let xy = rx!(x, y, |x, y| x * y);
//!
//!     // Set the value of `x` to 2 and trigger a re-calculation of `y` and `xy`.
//!     x.set(2);
//!     assert_eq!(x.get(), 2);
//!     assert_eq!(y.get(), 3);
//!     assert_eq!(xy.get(), 6);
//!
//!     // Observe changes to reactive variables.
//!     xy.observe(|n| { println!("xy is {}", n); });
//!     x.set(3); // prints "xy is 12"
//! }
//! ```
//!
//! ## Reactive Variables
//!
//! **Reactive Variables** (`Rx`) are values that can change over time. `Rxst` provides two kinds of Rx:
//!
//! * `RxCell` is a reactive cell that can be set. They act as the origin of changes in a reactive system from external sources.
//! * `RxNode` represents a reactive computation that depends on other Rxs. It is re-calculated when any of its dependencies are updated.
//!
//! ## Observers
//!
//! Each Rx may also be listened for changes with `observe`. This is useful for performing side effects in response to changes of the reactive value. Note that the registered closure must outlive the Rx.
//!
//! While it is not required or enforced, it is recommended that you only perform side effects in observers while the computation in `RxNode` remains side-effect free.
//!
//! ## Dataflow Graph
//!
//! Rxs and their dependencies form a *directed acyclic graph* (DAG) where the nodes of the DAG are Rxs and the edges are the dataflow between Rxs.
//!
//! To avoid inconsistent states (i.e. *glitches*), Rxs in a dataflow graph are updated in topological order:
//!
//! ```
//! # #[macro_use]
//! # extern crate rxst;
//! # use rxst::*;
//! # fn main() {
//! let a = RxCell::new(0);
//! let b = rx!(a, |a| a + 1);
//! let c = rx!(a, |a| a + 1);
//!
//! // `d` is always updated after `b` and `c`. A naive implementation may
//! // have `b` trigger `d` before `c` is updated and thus violate the
//! // invariant assumed by `d`.
//! let d = rx!(b, c, |b, c| assert_eq!(b, c));
//!
//! a.set(42);
//! # }
//! ```
//!
//! ## Lifetime
//!
//! Rx in Rxst are `Box`ed as the implementation uses raw pointers under the hood to avoid
//! reference counting, and that requires the address of each Rx to be stable. Rxst also annotates
//! each Rx so that a node cannot outlive its dependencies, and that each node is properly
//! unregistered from its sources when dropped. This also ensures that you can't
//! construct a cyclic dependency between Rxs.
//!
//! ```
//! # #[macro_use]
//! # extern crate rxst;
//! # use rxst::*;
//! # fn main() {
//! let a = RxCell::new(0);
//! {
//!     let b = rx!(a, |a| a + 1);
//!     b.observe(|b| println!("b is {}", b));
//!     a.set(1);
//!     a.set(2);
//! } // b is dropped here
//!
//! a.set(3); // prints nothing
//! # }
//! ```
//!
//! # Limitations
//!
//! * The value in Rx is currently limited to `T: Copy + 'static`.
//!
//! * Rxs cannot be empty and they also do not *terminate*. You can partially model empty Rxs with `Option<T>`.
//!
//! * `RxNode` can only depend on at most 4 Rxs as arguments, although this is simply a limitation of the implementation and can be easily increased.
//!
//! * Observers cannot be unregistered from Rxs yet.
//!
//! * `Rxst` currently only offer the most basic combinators for Rxs. More shall follow in the near future (merging, filtering, etc).

#![feature(nll)]
#![feature(vec_remove_item)]
#![feature(fn_traits)]
#![feature(unboxed_closures)]
#![warn(missing_docs)]

extern crate topological_sort;
use topological_sort::TopologicalSort;

use std::cell::Cell;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;

/// A value that can change over time.
pub trait Rx<'a> {
    /// The type of the value in Rx.
    type Type;

    /// Retrieve the value of the Rx.
    fn get(&self) -> Self::Type;

    /// Observe changes to Rx. The callback `F` is executed every time this Rx is updated, and the
    /// callback must outlive the Rx.
    fn observe<F>(&self, F)
    where
        F: Fn(Self::Type) -> () + Sized + 'a,
        Self: Sized,
    {
        unimplemented!();
    }

    #[doc(hidden)]
    unsafe fn add_node(&self, &Triggerable);

    #[doc(hidden)]
    fn remove_node(&self, &Triggerable);
}

#[doc(hidden)]
impl<'a, T0, Rx0: Rx<'a, Type = T0>> Rx<'a> for (&'a Rx0,) {
    type Type = (T0,);

    fn get(&self) -> Self::Type {
        (self.0.get(),)
    }

    unsafe fn add_node(&self, node: &Triggerable) {
        self.0.add_node(node);
    }

    fn remove_node(&self, node: &Triggerable) {
        self.0.remove_node(node);
    }
}

#[doc(hidden)]
impl<'a, T0, T1, Rx0: Rx<'a, Type = T0>, Rx1: Rx<'a, Type = T1>> Rx<'a> for (&'a Rx0, &'a Rx1) {
    type Type = (T0, T1);

    fn get(&self) -> Self::Type {
        (self.0.get(), self.1.get())
    }

    unsafe fn add_node(&self, node: &Triggerable) {
        self.0.add_node(node);
        self.1.add_node(node);
    }

    fn remove_node(&self, node: &Triggerable) {
        self.0.remove_node(node);
        self.1.remove_node(node);
    }
}

#[doc(hidden)]
impl<'a, T0, T1, T2, Rx0: Rx<'a, Type = T0>, Rx1: Rx<'a, Type = T1>, Rx2: Rx<'a, Type = T2>> Rx<'a> for (&'a Rx0, &'a Rx1, &'a Rx2) {
    type Type = (T0, T1, T2);

    fn get(&self) -> Self::Type {
        (self.0.get(), self.1.get(), self.2.get())
    }

    unsafe fn add_node(&self, node: &Triggerable) {
        self.0.add_node(node);
        self.1.add_node(node);
        self.2.add_node(node);
    }

    fn remove_node(&self, node: &Triggerable) {
        self.0.remove_node(node);
        self.1.remove_node(node);
        self.2.remove_node(node);
    }
}

#[doc(hidden)]
impl<'a, T0, T1, T2, T3, Rx0: Rx<'a, Type = T0>, Rx1: Rx<'a, Type = T1>, Rx2: Rx<'a, Type = T2>, Rx3: Rx<'a, Type = T3>> Rx<'a> for (&'a Rx0, &'a Rx1, &'a Rx2, &'a Rx3) {
    type Type = (T0, T1, T2, T3);

    fn get(&self) -> Self::Type {
        (self.0.get(), self.1.get(), self.2.get(), self.3.get())
    }

    unsafe fn add_node(&self, node: &Triggerable) {
        self.0.add_node(node);
        self.1.add_node(node);
        self.2.add_node(node);
        self.3.add_node(node);
    }

    fn remove_node(&self, node: &Triggerable) {
        self.0.remove_node(node);
        self.1.remove_node(node);
        self.2.remove_node(node);
        self.3.remove_node(node);
    }
}

/// A Rx that can be triggered to update its contents and notify its children.
pub trait Triggerable {
    /// Trigger the Rx and all of its downstream Rx, causing all registered callback to fire.
    fn trigger(&self);

    #[doc(hidden)]
    fn collect_dependencies(&self, &mut TopologicalSort<*const Triggerable>);
}

/// A reactive cell that can be set.
pub struct RxCell<'a, T: 'static> {
    val: Cell<T>,
    nodes: Cell<Vec<*const Triggerable>>,
    observers: Cell<Vec<Box<Fn(T) -> () + 'a>>>,
}

impl<'a, T: Copy> RxCell<'a, T> {
    /// Construct a new `RxCell` with the content `v`. The result is boxed as the address needs to
    /// be stable.
    pub fn new(v: T) -> Box<RxCell<'a, T>> {
        Box::new(RxCell {
            val: Cell::new(v),
            nodes: Cell::new(vec![]),
            observers: Cell::new(vec![]),
        })
    }

    /// Set the content of `RxCell` and trigger all Rxs that depend on it to update.
    pub fn set(&self, v: T) {
        self.val.set(v);

        self.trigger();

        let mut ts = TopologicalSort::<*const Triggerable>::new();
        self.collect_dependencies(&mut ts);

        while let Some(n) = ts.pop() {
            if n != self {
                unsafe {
                    (*n).trigger();
                }
            }
        }

        if ts.pop().is_none() {
            assert!(ts.is_empty(), "cycle detected");
        }
    }

    fn collect_dependencies_impl(
        &self,
        parent: *const Triggerable,
        ts: &mut TopologicalSort<*const Triggerable>,
    ) {
        let vec: &Vec<*const Triggerable> = unsafe { &*self.nodes.as_ptr() };
        for n in vec {
            ts.add_dependency(parent, *n as *const Triggerable);
            unsafe {
                (**n).collect_dependencies(ts);
            }
        }
    }
}

impl<'a, T: Copy + Display> Display for RxCell<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

impl<'a, T: Copy> Rx<'a> for RxCell<'a, T> {
    type Type = T;

    fn get(&self) -> T {
        self.val.get()
    }

    #[doc(hidden)]
    unsafe fn add_node(&self, n: &Triggerable) {
        with_cell(&self.nodes, |nodes| nodes.push(n));
    }

    #[doc(hidden)]
    fn remove_node(&self, n: &Triggerable) {
        with_cell(&self.nodes, |nodes| {
            nodes.remove_item(&(n as *const Triggerable));
        });
    }

    fn observe<F>(&self, f: F)
    where
        F: Fn(T) -> () + Sized + 'a,
    {
        with_cell(&self.observers, move |observers| {
            observers.push(Box::new(f));
        });
    }
}

impl<'a, T: Copy> Triggerable for RxCell<'a, T> {
    fn trigger(&self) {
        // This can invalidate the `vec` if any of the observers happen
        // to be attaching observers to this very RxCell. Fortunately,
        // this cannot happen as the observer cannot borrow the RxCell it
        // is listening on.
        for o in unsafe { &*self.observers.as_ptr() } {
            o(self.get());
        }
    }

    #[doc(hidden)]
    fn collect_dependencies(&self, ts: &mut TopologicalSort<*const Triggerable>) {
        self.collect_dependencies_impl(self as &Triggerable, ts);
    }
}

/// A reactive value that depends on other Rxs.
///
/// This reads as: a reactive computation `F` from `Args` to `T` that depends on references to
/// `RxArgs` (with lifetime `'a`).
pub struct RxNode<'a, T: Copy + 'static, Args, RxArgs: Rx<'a, Type = Args>, F: Fn<Args, Output = T>>
{
    var: RxCell<'a, T>,
    f: F,
    rxargs: RxArgs,
    _args: PhantomData<Args>,
}

#[doc(hidden)]
fn rxn<'a, T: Copy + 'static, Args, RxArgs, F>(
    rxargs: RxArgs,
    f: F
) -> Box<RxNode<'a, T, Args, RxArgs, F>>
where
    F: Fn<Args, Output = T> + 'a,
    RxArgs: Rx<'a, Type = Args>
{
    let node = Box::new(RxNode {
        var: *RxCell::new(f.call(rxargs.get())),
        f: f,
        rxargs: rxargs,
        _args: PhantomData,
    });
    let p = Box::into_raw(node);
    unsafe {
        ((*p).rxargs).add_node(&*p);
    }
    unsafe { Box::from_raw(p) }
}

#[doc(hidden)]
pub fn rx1<'a, 'b, T: Copy + 'static, T0, Rx0, F>(
    rx0: &'b Rx0,
    f: F,
) -> Box<RxNode<'a, T, (T0,), (&'a Rx0,), F>>
where
    F: Fn<(T0,), Output = T> + 'a,
    Rx0: Rx<'a, Type = T0>,
    'a: 'b,
{
    rxn(unsafe { (&*(rx0 as *const Rx0),) }, f)
}

#[doc(hidden)]
pub fn rx2<'a, 'b, T: Copy + 'static, T0, Rx0, T1, Rx1, F>(
    rx0: &'b Rx0,
    rx1: &'b Rx1,
    f: F,
) -> Box<RxNode<'a, T, (T0, T1), (&'a Rx0, &'a Rx1), F>>
where
    F: Fn<(T0, T1), Output = T> + 'a,
    Rx0: Rx<'a, Type = T0>,
    Rx1: Rx<'a, Type = T1>,
    'a: 'b,
{
    rxn(unsafe { (&*(rx0 as *const Rx0), &*(rx1 as *const Rx1)) }, f)
}

#[doc(hidden)]
pub fn rx3<'a, 'b, T: Copy + 'static, T0, Rx0, T1, Rx1, T2, Rx2, F>(
    rx0: &'b Rx0,
    rx1: &'b Rx1,
    rx2: &'b Rx2,
    f: F,
) -> Box<RxNode<'a, T, (T0, T1, T2), (&'a Rx0, &'a Rx1, &'a Rx2), F>>
where
    F: Fn<(T0, T1, T2), Output = T> + 'a,
    Rx0: Rx<'a, Type = T0>,
    Rx1: Rx<'a, Type = T1>,
    Rx2: Rx<'a, Type = T2>,
    'a: 'b,
{
    rxn(unsafe { (&*(rx0 as *const Rx0), &*(rx1 as *const Rx1), &*(rx2 as *const Rx2)) }, f)
}

#[doc(hidden)]
pub fn rx4<'a, 'b, T: Copy + 'static, T0, Rx0, T1, Rx1, T2, Rx2, T3, Rx3, F>(
    rx0: &'b Rx0,
    rx1: &'b Rx1,
    rx2: &'b Rx2,
    rx3: &'b Rx3,
    f: F,
) -> Box<RxNode<'a, T, (T0, T1, T2, T3), (&'a Rx0, &'a Rx1, &'a Rx2, &'a Rx3), F>>
where
    F: Fn<(T0, T1, T2, T3), Output = T> + 'a,
    Rx0: Rx<'a, Type = T0>,
    Rx1: Rx<'a, Type = T1>,
    Rx2: Rx<'a, Type = T2>,
    Rx3: Rx<'a, Type = T3>,
    'a: 'b,
{
    rxn(unsafe { (&*(rx0 as *const Rx0), &*(rx1 as *const Rx1), &*(rx2 as *const Rx2), &*(rx3 as *const Rx3)) }, f)
}

impl<'a, T: Copy, Args, RxArgs: Rx<'a, Type = Args>, F> Drop for RxNode<'a, T, Args, RxArgs, F>
where
    F: Fn<Args, Output = T>,
{
    fn drop(&mut self) {
        self.rxargs.remove_node(self);
    }
}

impl<'a, T: Copy + Display, Args, RxArgs: Rx<'a, Type = Args>, F: Fn<Args, Output = T>> Display
    for RxNode<'a, T, Args, RxArgs, F>
{
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        self.var.fmt(f)
    }
}

impl<'a, T: Copy, Args, RxArgs: Rx<'a, Type = Args>, F> Rx<'a> for RxNode<'a, T, Args, RxArgs, F>
where
    F: Fn<Args, Output = T>,
{
    type Type = T;

    fn get(&self) -> T {
        self.var.get()
    }

    #[doc(hidden)]
    unsafe fn add_node(&self, n: &Triggerable) {
        self.var.add_node(n);
    }

    #[doc(hidden)]
    fn remove_node(&self, n: &Triggerable) {
        self.var.remove_node(n);
    }

    fn observe<G>(&self, g: G)
    where
        G: Fn(T) -> () + Sized + 'a,
    {
        self.var.observe(g);
    }
}

impl<'a, T: Copy + 'static, Args, RxArgs, F> Triggerable for RxNode<'a, T, Args, RxArgs, F>
where
    RxArgs: Rx<'a, Type = Args>,
    F: Fn<Args, Output = T>,
{
    fn trigger(&self) {
        let v = (self.f).call(self.rxargs.get());
        self.var.val.set(v);
        self.var.trigger();
    }

    #[doc(hidden)]
    fn collect_dependencies(&self, ts: &mut TopologicalSort<*const Triggerable>) {
        self.var.collect_dependencies_impl(self as &Triggerable, ts);
    }
}

// Modify the content of a cell during which the cell is temporarily swapped with `T::default()`.
fn with_cell<T: Default, F>(cell: &Cell<T>, f: F)
where
    F: FnOnce(&mut T) -> (),
{
    let mut x = cell.take();
    f(&mut x);
    cell.set(x);
}

/// Construct a Rx from its dependenices.
///
/// This macro has two forms:
///
/// * `rx!(e)` returns a `RxCell` with the value `e`. It is a shorthand for `RxCell::new(e)`.
/// * `rx!(rx0, ..., rxn, f)` takes Rxs `rx0: Box<Rx<T0>>`, ..., `rxn: Box<Rx<TN>>`, and a function
/// `f: Fn(T0, ..., TN) -> R`. It returns a `RxNode` of type `R` whose value is computed using `f` with values
/// from `rx0`, ... `rxn`. The function `f` must outlive all Rxs it depends on.
///
/// See the top-level [documentation](index.html#Example) for examples.
#[macro_export]
macro_rules! rx {
    ($e:expr) => {{ RxCell::new($e) }};
    ($a:expr, $fn:expr) => {{ rx1($a.as_ref(), $fn) }};
    ($a:expr, $b:expr, $fn:expr) => {{ rx2($a.as_ref(), $b.as_ref(), $fn) }};
    ($a:expr, $b:expr, $c:expr, $fn:expr) => {{ rx3($a.as_ref(), $b.as_ref(), $c.as_ref(), $fn) }};
    ($a:expr, $b:expr, $c:expr, $d: expr, $fn:expr) => {{ rx4($a.as_ref(), $b.as_ref(), $c.as_ref(), $d.as_ref(), $fn) }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_display() {
        let x = RxCell::new(42);
        let y = rx!(x, |x| x);
        println!("{} {} ", x, y);
    }

    #[test]
    fn rx_macro() {
        let a = rx!(());
        let b = rx!(a, |_| ());
        let c = rx!(a, b, |_, _| ());
        let d = rx!(a, b, c, |_, _, _| ());
        let _e = rx!(a, b, c, d, |_, _, _, _| ());
    }
}
