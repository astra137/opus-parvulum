mod controller;
mod dsp;
mod params;
mod processor;

use audiopus::Bandwidth;
use std::os::raw::c_void;

pub use controller::OpusController;
pub use processor::OpusProcessor;

pub struct ContextPtr(*mut c_void);

#[derive(Debug)]
pub struct SaveState {
	bypass: bool,
	complexity: u8,
	gain: i32,
	inband_fec: bool,
	max_bandwidth: Bandwidth,
	packet_loss_perc: u8,
}

impl SaveState {
	fn new() -> SaveState {
		SaveState {
			bypass: false,
			complexity: 9,
			gain: 0,
			inband_fec: false,
			max_bandwidth: Bandwidth::Fullband,
			packet_loss_perc: 0,
		}
	}
}
