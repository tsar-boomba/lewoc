use embassy_futures::select::{Either, select};
use embassy_rp::gpio::Input;
use embassy_sync::{blocking_mutex::raw::RawMutex, signal::Signal};
use embassy_time::{Duration, Timer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Good,
    Help,
}

pub async fn task<'a, M: RawMutex>(
    signal: &'a Signal<M, Button>,
    mut good_in: Input<'a>,
    mut help_in: Input<'a>,
) {
    loop {
        let good_low = good_in.wait_for_falling_edge();
        let help_low = help_in.wait_for_falling_edge();

        match select(good_low, help_low).await {
            Either::First(()) => {
                signal.signal(Button::Good);
            }
            Either::Second(()) => {
                signal.signal(Button::Help);
            }
        }

        // Debounce successful press
        Timer::after(Duration::from_millis(250)).await;
    }
}
