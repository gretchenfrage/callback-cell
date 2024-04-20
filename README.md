Like an `Atomic<Option<Box<dyn FnOnce + Send + 'static>>>`.

This is a barebones concurrency utility that is useful for building larger
abstractions on top of. For example, the `seg_queue` example shows how this
can be used to elevant crossbeam's `SegQueue` (a concurrent queue which does
not support blocking, only polling) into an mpsc queue which supports both
blocking and async/await:


```rust,ignore
pub struct Sender<T>(Arc<State<T>>);

pub struct Receiver<T>(Arc<State<T>>);

struct State<T> {
	queue: SegQueue<T>,
	callback_cell: CallbackCell,
}

fn new_queue<T>() -> (Sender<T>, Receiver<T>) {
	let state_1 = Arc::new(State {
		queue: SegQueue::new(),
		callback_cell: CallbackCell::new(),
	});
	let state_2 = Arc::clone(&state_1);
	(Sender(state_1), Receiver(state_2))
}

impl<T> Sender<T> {
	fn send(&self, item: T) {
		self.0.queue.push(item);
		self.0.callback_cell.take_call();
	}
}

impl<T> Receiver<T> {
	fn recv_blocking(&mut self) -> T {
		if let Some(item) = self.0.queue.pop() {
			return item;
		}
		let parker = Parker::new();
		loop {
			let unparker = parker.unparker().clone();
			self.0.callback_cell.put(move || unparker.unpark());
			if let Some(item) = self.0.queue.pop() {
				return item;
			}
			parker.park();
		}
	}

	async fn recv_async(&mut self) -> T {
		if let Some(item) = self.0.queue.pop() {
			return item;
		}
		let notify_1 = Arc::new(Notify::new());
		loop {
			let notify_2 = Arc::clone(&notify_1);
			self.0.callback_cell.put(move || notify_2.notify_one());
			if let Some(item) = self.0.queue.pop() {
				return item;
			}
			notify_1.notified().await;
		}
	}
}
```

A naive way of implementing this would involve two layers of
indirection:

- First, the `FnOnce` could be boxed into a `Box<dyn FnOnce>`, achieving
  dynamic dispatch.
- Then, that could be boxed into a `Box<Box<dyn FnOnce>>`, making it a
  normal pointer rather than a fat pointer.
- That outer `Box` could be converted into a raw pointer and then into a
  `usize` and stored in an `AtomicUsize`.

This utility, however, does this in only one heap allocation rather than
two, through slightly clever usage of monomorphization and the `std::alloc`
API.
