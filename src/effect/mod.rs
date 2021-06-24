mod controller;
mod dsp;
mod params;
mod processor;

use std::os::raw::c_void;
use vst3_com::IID;

pub use controller::OpusController;
pub use processor::OpusProcessor;

pub struct ContextPtr(*mut c_void);

pub struct VstClassInfo {
	pub cid: IID,
	pub name: &'static str,
	pub category: &'static str,
	pub subcategories: &'static str,
	pub class_flags: u32,
	pub cardinality: i32,
}
