
use crate::*;
use std::{
    sync::{
        Arc,
        atomic::{
            AtomicU32,
            Ordering,
        },
    },
    ops::Add,
};

#[test]
fn without_args_test() {
    let counter = Arc::new(AtomicU32::new(0));
    let cell = CallbackCell::new();
    assert_eq!(counter.load(Ordering::Relaxed), 0);
    assert!(!cell.take_call());
    assert_eq!(counter.load(Ordering::Relaxed), 0);
    cell.put({
        let counter = Arc::clone(&counter);
        move || {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    });
    assert_eq!(counter.load(Ordering::Relaxed), 0);
    assert!(cell.take_call());
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    assert!(!cell.take_call());
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    cell.put({
        struct DropGuardThing(Arc<AtomicU32>);
        impl Drop for DropGuardThing {
            fn drop(&mut self) {
                self.0.fetch_add(100, Ordering::Relaxed);
            }
        }
        let dgt = DropGuardThing(Arc::clone(&counter));
        move || {
            dgt.0.fetch_add(2, Ordering::Relaxed);
        }
    });
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    cell.put({
        let counter = Arc::clone(&counter);
        move || {
            counter.fetch_add(3, Ordering::Relaxed);
        }
    });
    assert_eq!(counter.load(Ordering::Relaxed), 101);
    assert!(cell.take_call());
    assert_eq!(counter.load(Ordering::Relaxed), 104);
    assert!(!cell.take_call());
    assert_eq!(counter.load(Ordering::Relaxed), 104);
}

#[test]
fn with_args_test() {
    #[derive(Debug)]
    struct Thing(i32, &'static AtomicU32);
    impl From<i32> for Thing {
        fn from(n: i32) -> Self {
            Thing(n, Box::leak(Box::new(AtomicU32::new(0))))
        }
    }
    impl Drop for Thing {
        fn drop(&mut self) {
            // panic on double free
            assert_eq!(self.1.fetch_add(1, Ordering::Relaxed), 0);
        }
    }
    impl Add<i32> for Thing {
        type Output = Self;
        fn add(self, rhs: i32) -> Self {
            Self::from(self.0 + rhs)
        }
    }

    let counter = Arc::new(AtomicU32::new(0));
    let cell: CallbackCellArgs<Thing, Thing> = CallbackCellArgs::new();
    assert_eq!(counter.load(Ordering::Relaxed), 0);
    assert_eq!(cell.take_call(Thing::from(10000)).unwrap_err().0, 10000);
    assert_eq!(counter.load(Ordering::Relaxed), 0);
    cell.put({
        let counter = Arc::clone(&counter);
        move |i| {
            counter.fetch_add(1, Ordering::Relaxed);
            i + 1000
        }
    });
    assert_eq!(counter.load(Ordering::Relaxed), 0);
    assert_eq!(cell.take_call(Thing::from(20000)).unwrap().0, 21000);
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    assert_eq!(cell.take_call(Thing::from(30000)).unwrap_err().0, 30000);
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    cell.put({
        struct DropGuardThing(Arc<AtomicU32>);
        impl Drop for DropGuardThing {
            fn drop(&mut self) {
                self.0.fetch_add(100, Ordering::Relaxed);
            }
        }
        let dgt = DropGuardThing(Arc::clone(&counter));
        move |i| {
            dgt.0.fetch_add(2, Ordering::Relaxed);
            i + 2000
        }
    });
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    cell.put({
        let counter = Arc::clone(&counter);
        move |i| {
            counter.fetch_add(3, Ordering::Relaxed);
            i + 3000
        }
    });
    assert_eq!(counter.load(Ordering::Relaxed), 101);
    assert_eq!(cell.take_call(Thing::from(40000)).unwrap().0, 43000);
    assert_eq!(counter.load(Ordering::Relaxed), 104);
    assert_eq!(cell.take_call(Thing::from(50000)).unwrap_err().0, 50000);
    assert_eq!(counter.load(Ordering::Relaxed), 104);
}

