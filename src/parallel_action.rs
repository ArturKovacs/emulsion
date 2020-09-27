use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::Duration;

enum ActionState<InpT, OutT> {
	Ready,
	InputGiven(InpT),
	Pending,
	OutputReady(OutT),
}

struct Shared<InpT, OutT> {
	run: AtomicBool,
	state: Mutex<ActionState<InpT, OutT>>,
}

// struct Processor<InpT, OutT> {
//     shared: Arc<Shared<InpT, OutT>>,
// }
// impl<InpT, OutT> Processor<InpT, OutT> {
//     fn run<F: FnMut(InpT)->OutT>(&mut self, action: F) {

//     }
// }

fn run_processor<InpT, OutT, F: FnMut(InpT) -> OutT>(
	shared: Arc<Shared<InpT, OutT>>,
	mut action: F,
) {
	while shared.run.load(Ordering::SeqCst) {
		// Sleep to prevent buisy waiting
		std::thread::sleep(Duration::from_millis(5));
		let input = {
			let mut state = shared.state.lock().unwrap();
			let has_input = match &*state {
				ActionState::InputGiven(_) => true,
				_ => false,
			};
			if has_input {
				let mut input = ActionState::Pending;
				std::mem::swap(&mut input, &mut *state);
				match input {
					ActionState::InputGiven(inp) => Some(inp),
					_ => unreachable!(),
				}
			} else {
				None
			}
		}; // let go of the mutex lock
		if let Some(input) = input {
			let output = action(input);
			let mut state = shared.state.lock().unwrap();
			let got_new_request = match &*state {
				ActionState::Pending => false,
				_ => true,
			};
			if !got_new_request {
				*state = ActionState::OutputReady(output);
			}
		}
	}
}

pub struct ParallelAction<InpT, OutT> {
	shared: Arc<Shared<InpT, OutT>>,
	join_handle: Option<JoinHandle<()>>,
}

impl<InpT: Send + 'static, OutT: Send + 'static> ParallelAction<InpT, OutT> {
	pub fn new<F: 'static + Send + FnMut(InpT) -> OutT>(action: F) -> ParallelAction<InpT, OutT> {
		let shared =
			Arc::new(Shared { run: AtomicBool::new(true), state: Mutex::new(ActionState::Ready) });
		let handle = {
			// let mut processor = Processor {
			//     shared: shared.clone(),
			// };
			let shared = shared.clone();
			std::thread::spawn(move || {
				run_processor(shared, action);
			})
		};

		ParallelAction { join_handle: Some(handle), shared }
	}

	/// Returns the given input if the thread is currently occupied
	pub fn give_input(&self, input: InpT) {
		let mut state = self.shared.state.lock().unwrap();
		*state = ActionState::InputGiven(input);
	}

	pub fn try_get_output(&self) -> Option<OutT> {
		let mut state = self.shared.state.lock().unwrap();
		let has_output = match &*state {
			ActionState::OutputReady(_) => true,
			_ => false,
		};
		if has_output {
			let mut output = ActionState::Ready;
			std::mem::swap(&mut output, &mut *state);
			match output {
				ActionState::OutputReady(o) => Some(o),
				_ => unreachable!(),
			}
		} else {
			None
		}
	}

	pub fn is_ready(&self) -> bool {
		let state = self.shared.state.lock().unwrap();
		match &*state {
			ActionState::Ready => true,
			_ => false,
		}
	}
}
impl<InpT, OutT> Drop for ParallelAction<InpT, OutT> {
	fn drop(&mut self) {
		self.shared.run.store(false, Ordering::SeqCst);
		if let Some(handle) = self.join_handle.take() {
			handle.join().unwrap();
		}
	}
}
