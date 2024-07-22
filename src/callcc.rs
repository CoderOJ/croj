use std::{
	future::Future,
	pin::Pin,
	sync::{Arc, Mutex},
	task::{Context, Poll},
};

pub struct KEntrance<T> {
	state: Arc<Mutex<KEntranceState<T>>>,
}

impl<T> KEntrance<T> {
	pub fn resume(self, value: T) {
		let mut state = self.state.lock().unwrap();
		state.value = Poll::Ready(value);
		if let Some(waker) = &state.waker {
			waker.clone().wake();
		}
	}
}

// why cannot auto generate clone trait?
impl<T> Clone for KEntrance<T> {
	fn clone(&self) -> Self {
		return Self {
			state: self.state.clone(),
		};
	}
}

pub struct KEntranceState<T> {
	value: std::task::Poll<T>,
	waker: Option<std::task::Waker>,
}

impl<T> Future for KEntrance<T> {
	type Output = T;
	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let mut state = self.state.lock().unwrap();
		if state.value.is_ready() {
			// just taking state.value since this future will never be polled again
			return std::mem::replace(&mut state.value, std::task::Poll::Pending);
		} else {
			state.waker = Some(cx.waker().clone());
			return std::task::Poll::Pending;
		}
	}
}

pub fn callcc<T>(f: impl FnOnce(KEntrance<T>)) -> KEntrance<T> {
	let k = KEntrance {
		state: Arc::new(Mutex::new(KEntranceState {
			value: Poll::Pending,
			waker: None,
		})),
	};
	f(k.clone());
	return k;
}

/// callcc with return value
/// instead of Option<T>, use Result<(), T> to support ? operator
/// requires returning Ok(()) at the end
pub fn callcc_ret<T>(f: impl FnOnce(KEntrance<T>) -> Result<(), T>) -> KEntrance<T> {
	let k = KEntrance {
		state: Arc::new(Mutex::new(KEntranceState {
			value: Poll::Pending,
			waker: None,
		})),
	};
	return match f(k.clone()) {
		Ok(_) => k,
		Err(val) => KEntrance {
			state: Arc::new(Mutex::new(KEntranceState {
				value: Poll::Ready(val),
				waker: None,
			})),
		},
	};
}
