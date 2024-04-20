//! Example of using `CallbackCell` to elevate crossbeam `SegQueue` (a concurrent queue which does
//! not support blocking, only polling) into an mpsc queue which supports both blocking and
//! async-awaiting.

use callback_cell::CallbackCell;
use std::{
	sync::Arc,
	thread::{
		self,
		sleep,
	},
	time::Duration,
};
use crossbeam::{
	queue::SegQueue,
	sync::Parker,
};
use tokio::{
	sync::Notify,
	runtime::Runtime,
};


/// Sender half to mpsc queue.
pub struct Sender<T>(Arc<State<T>>);

/// Receiver half to mpsc queue.
pub struct Receiver<T>(Arc<State<T>>);

struct State<T> {
	queue: SegQueue<T>,
	callback_cell: CallbackCell,
}

/// Create an mpsc queue.
fn new_queue<T>() -> (Sender<T>, Receiver<T>) {
	let state_1 = Arc::new(State {
		queue: SegQueue::new(),
		callback_cell: CallbackCell::new(),
	});
	let state_2 = Arc::clone(&state_1);
	(Sender(state_1), Receiver(state_2))
}

impl<T> Sender<T> {
	/// Send item into queue.
	fn send(&self, item: T) {
		self.0.queue.push(item);
		self.0.callback_cell.take_call();
	}
}

impl<T> Clone for Sender<T> {
	fn clone(&self) -> Self {
		Sender(Arc::clone(&self.0))
	}
}

impl<T> Receiver<T> {
	/// Take item from queue, blocking until one is present.
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

	/// Future to take item from queue, yielding until one is present.
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

fn main() {
	let (send, mut recv) = new_queue();
	thread::spawn(move || {
		for i in 0..10 {
			send.send(i);
			sleep(Duration::from_millis(100));
		}
	});
	for _ in 0..5 {
		let i = recv.recv_blocking();
		println!("received through blocking: {}", i);
	}
	println!("entering tokio runtime");
	Runtime::new().unwrap().block_on(async move {
		for _ in 0..5 {
			let i = recv.recv_async().await;
			println!("receiving through awaiting: {}", i);
		}
	});
}
