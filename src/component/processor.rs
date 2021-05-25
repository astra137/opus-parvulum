use super::dsp;
use super::ContextPtr;
use super::SaveState;
use crate::vst_str;
use crate::Component;
use hex_literal::hex;
use log::*;
use std::cell::RefCell;
use std::cell::RefMut;
use std::mem::size_of;
use std::ptr::null_mut;
use std::slice;
use vst3_com::{c_void, sys::GUID, ComPtr, IID};
use vst3_sys::base::kInvalidArgument;
use vst3_sys::base::ClassCardinality;
use vst3_sys::vst::BusDirections;
use vst3_sys::vst::MediaTypes;
use vst3_sys::vst::SpeakerArrangement;
use vst3_sys::vst::{
	BusDirection, BusInfo, BusType, IAudioProcessor, IComponent, IEventList, IoMode, MediaType,
	ProcessData, ProcessSetup, RoutingInfo, K_SAMPLE32, K_SAMPLE64,
};
use vst3_sys::VST3;
use vst3_sys::{
	base::{
		kInternalError, kNotImplemented, kNotInitialized, kResultFalse, kResultOk, kResultTrue,
		tresult, IBStream, IPluginBase, TBool,
	},
	vst::kStereo,
};

macro_rules! graceful {
	($expr:expr) => {
		match $expr {
			Ok(x) => x,
			Err(err) => {
				error!("{}", err);
				return kInternalError;
			}
		}
	};
}

// TODO add repr(i32) to MediaTypes and BusDirections, maybe?
const KAUDIO: MediaType = MediaTypes::kAudio as MediaType;
const KEVENT: MediaType = MediaTypes::kEvent as MediaType;
const KINPUT: MediaType = BusDirections::kInput as BusDirection;
const KOUTPUT: MediaType = BusDirections::kOutput as BusDirection;

pub struct AudioBus {
	name: [i16; 128],
	bus_type: BusType,
	flags: i32,
	active: TBool,
	speaker_arr: SpeakerArrangement,
}

struct CurrentProcessorMode(i32);
struct ProcessSetupWrapper(ProcessSetup);
struct AudioInputs(Vec<AudioBus>);
struct AudioOutputs(Vec<AudioBus>);

#[VST3(implements(IComponent, IAudioProcessor))]
pub struct OpusProcessor {
	current_process_mode: RefCell<CurrentProcessorMode>,
	process_setup: RefCell<ProcessSetupWrapper>,
	audio_inputs: RefCell<AudioInputs>,
	audio_outputs: RefCell<AudioOutputs>,
	context: RefCell<ContextPtr>,
	save_state: RefCell<SaveState>,
	opus_dsp: RefCell<Option<Box<dsp::OpusDsp>>>,
}

impl Component for OpusProcessor {
	const CID: GUID = GUID {
		data: hex!("8eea3bf524c346238e5bbe445f88b05a"),
	};
	const CARDINALITY: i32 = ClassCardinality::kManyInstances as i32;
	const CATEGORY: &'static str = "Audio Module Class";
	const NAME: &'static str = "Opus Parvulum";
	const CLASS_FLAGS: u32 = 1; // 1 distributable, 2 simple io supported
	const SUBCATEGORIES: &'static str = "Fx";
}

impl OpusProcessor {
	pub fn new() -> Box<Self> {
		let current_process_mode = RefCell::new(CurrentProcessorMode(0));
		let process_setup = RefCell::new(ProcessSetupWrapper(ProcessSetup {
			process_mode: 0,
			symbolic_sample_size: 0,
			max_samples_per_block: 0,
			sample_rate: 0.0,
		}));
		let audio_inputs = RefCell::new(AudioInputs(vec![]));
		let audio_outputs = RefCell::new(AudioOutputs(vec![]));
		let context = RefCell::new(ContextPtr(null_mut()));
		let save_state = RefCell::new(SaveState::new());
		let opus_dsp = RefCell::new(None);
		Self::allocate(
			current_process_mode,
			process_setup,
			audio_inputs,
			audio_outputs,
			context,
			save_state,
			opus_dsp,
		)
	}

	pub fn create_instance() -> *mut c_void {
		Box::into_raw(Self::new()) as *mut c_void
	}

	pub unsafe fn add_audio_input(&self, name: &str, arr: SpeakerArrangement) {
		let new_bus = AudioBus {
			name: vst_str::str_16(name),
			bus_type: 0,
			flags: 1,
			active: false as u8,
			speaker_arr: arr,
		};
		self.audio_inputs.borrow_mut().0.push(new_bus);
	}

	pub unsafe fn add_audio_output(&self, name: &str, arr: SpeakerArrangement) {
		let new_bus = AudioBus {
			name: vst_str::str_16(name),
			bus_type: 0,
			flags: 1,
			active: false as u8,
			speaker_arr: arr,
		};
		self.audio_outputs.borrow_mut().0.push(new_bus);
	}
}

fn get_channel_count(arr: SpeakerArrangement) -> i32 {
	let mut arr = arr;
	let mut count = 0;
	while arr != 0 {
		if (arr & 1) == 1 {
			count += 1;
		}
		arr >>= 1;
	}
	count
}

impl IComponent for OpusProcessor {
	unsafe fn get_controller_class_id(&self, tuid: *mut IID) -> tresult {
		info!("get_controller_class_id()");
		*tuid = super::controller::OpusController::CID;
		kResultOk
	}

	unsafe fn set_io_mode(&self, mode: IoMode) -> tresult {
		info!("set_io_mode(mode: {})", mode);
		kNotImplemented
	}

	unsafe fn get_bus_count(&self, media_type: MediaType, dir: BusDirection) -> i32 {
		let result = match media_type {
			KAUDIO => match dir {
				KINPUT => self.audio_inputs.borrow().0.len() as i32,
				KOUTPUT => self.audio_outputs.borrow().0.len() as i32,
				_ => 0,
			},
			KEVENT => 0,
			_ => 0,
		};

		info!(
			"get_bus_count(media_type: {}, dir: {}) => {}",
			media_type, dir, result
		);
		result
	}

	unsafe fn get_bus_info(
		&self,
		media_type: MediaType,
		direction: BusDirection,
		index: i32,
		info: *mut BusInfo,
	) -> tresult {
		let info = &mut *info;

		let result = match media_type {
			KAUDIO => match direction {
				KINPUT => match self.audio_inputs.borrow().0.get(index as usize) {
					Some(bus) => {
						*info = BusInfo {
							media_type,
							direction,
							channel_count: get_channel_count(bus.speaker_arr),
							name: bus.name,
							bus_type: bus.bus_type,
							flags: bus.flags as u32,
						};

						kResultTrue
					}
					None => kInvalidArgument,
				},
				KOUTPUT => match self.audio_outputs.borrow().0.get(index as usize) {
					Some(bus) => {
						*info = BusInfo {
							media_type,
							direction,
							channel_count: get_channel_count(bus.speaker_arr),
							name: bus.name,
							bus_type: bus.bus_type,
							flags: bus.flags as u32,
						};

						kResultTrue
					}
					None => kInvalidArgument,
				},
				_ => kInvalidArgument,
			},
			KEVENT => kResultFalse,
			_ => kInvalidArgument,
		};

		info!(
			"get_bus_info(media_type: {}, dir: {}, index: {}) => {}",
			media_type,
			direction,
			index,
			result == 0
		);

		result
	}

	unsafe fn get_routing_info(
		&self,
		_in_info: *mut RoutingInfo,
		_out_info: *mut RoutingInfo,
	) -> tresult {
		info!("get_routing_info() => kNotImplemented");
		kNotImplemented
	}

	unsafe fn activate_bus(
		&self,
		media_type: MediaType,
		dir: BusDirection,
		index: i32,
		state: TBool,
	) -> tresult {
		info!(
			"activate_bus(media_type: {}, dir: {}, index: {}, state: {})",
			media_type, dir, index, state
		);

		let mut inputs = self.audio_inputs.borrow_mut();
		let mut outputs = self.audio_outputs.borrow_mut();

		match media_type {
			KAUDIO => match dir {
				KINPUT => match inputs.0.get_mut(index as usize) {
					Some(bus) => {
						bus.active = state;
						kResultTrue
					}
					None => kInvalidArgument,
				},
				KOUTPUT => match outputs.0.get_mut(index as usize) {
					Some(bus) => {
						bus.active = state;
						kResultTrue
					}
					None => kInvalidArgument,
				},
				_ => kInvalidArgument,
			},
			KEVENT => kResultFalse,
			_ => kInvalidArgument,
		}
	}

	unsafe fn set_active(&self, state: TBool) -> tresult {
		info!("set_active(state: {})", state);

		kResultOk
	}

	unsafe fn set_state(&self, state: *mut c_void) -> tresult {
		if state.is_null() {
			info!("set_state() => kResultFalse");
			return kResultFalse;
		}

		let state = state as *mut *mut _;
		let state: ComPtr<dyn IBStream> = ComPtr::new(state);

		let mut save_state = self.save_state.borrow_mut();

		let mut num_bytes_read = 0;
		let saved_params_ptr = &mut *save_state as *mut SaveState as *mut c_void;
		state.read(
			saved_params_ptr,
			size_of::<SaveState>() as i32,
			&mut num_bytes_read,
		);

		info!("set_state() => kResultOk, {:?}", save_state);
		kResultOk
	}

	unsafe fn get_state(&self, state: *mut c_void) -> tresult {
		info!("get_state()");

		if state.is_null() {
			return kResultFalse;
		}

		let state = state as *mut *mut _;
		let state: ComPtr<dyn IBStream> = ComPtr::new(state);

		let dsp = self.opus_dsp.borrow();
		let save_state = match &*dsp {
			Some(dsp) => SaveState {
				bypass: dsp.bypass,
				complexity: dsp.encoder.complexity().unwrap(),
				inband_fec: dsp.encoder.inband_fec().unwrap(),
				packet_loss_perc: dsp.encoder.packet_loss_perc().unwrap(),
				max_bandwidth: dsp.encoder.max_bandwidth().unwrap(),
				gain: dsp.decoder.gain().unwrap(),
			},
			None => SaveState::new(),
		};

		let mut num_bytes_written = 0;
		let saved_params_ptr = &save_state as *const SaveState as *const c_void;
		state.write(
			saved_params_ptr,
			size_of::<SaveState>() as i32,
			&mut num_bytes_written,
		);

		kResultOk
	}
}

impl IPluginBase for OpusProcessor {
	unsafe fn initialize(&self, context: *mut c_void) -> tresult {
		info!("initialize()");

		if !self.context.borrow().0.is_null() {
			return kResultFalse;
		}
		self.context.borrow_mut().0 = context;

		self.add_audio_input("Stereo In", kStereo);
		self.add_audio_output("Stereo Out", kStereo);

		kResultOk
	}

	unsafe fn terminate(&self) -> tresult {
		info!("terminate()");
		self.audio_inputs.borrow_mut().0.clear();
		self.audio_outputs.borrow_mut().0.clear();
		self.context.borrow_mut().0 = null_mut();
		kResultOk
	}
}

impl IAudioProcessor for OpusProcessor {
	unsafe fn set_bus_arrangements(
		&self,
		inputs: *mut SpeakerArrangement,
		num_ins: i32,
		outputs: *mut SpeakerArrangement,
		num_outs: i32,
	) -> tresult {
		// SAFETY: inputs and outputs are arrays of SpeakerArrangement
		let inputs = slice::from_raw_parts_mut(inputs, num_ins as usize);
		let outputs = slice::from_raw_parts_mut(outputs, num_outs as usize);

		info!("set_bus_arrangements({:?}, {:?}) => false", inputs, outputs);
		kResultFalse
	}

	unsafe fn get_bus_arrangement(
		&self,
		dir: BusDirection,
		index: i32,
		arr: *mut SpeakerArrangement,
	) -> tresult {
		// arr is a single SpeakerArrangement
		let arr = &mut *arr;

		let result = match dir {
			0 => {
				if index as usize >= self.audio_inputs.borrow().0.len() {
					kResultFalse
				} else {
					*arr = self.audio_inputs.borrow().0[index as usize].speaker_arr;
					kResultTrue
				}
			}
			_ => {
				if index as usize >= self.audio_outputs.borrow().0.len() {
					kResultFalse
				} else {
					*arr = self.audio_outputs.borrow().0[index as usize].speaker_arr;
					kResultTrue
				}
			}
		};

		info!(
			"get_bus_arrangements(dir: {}, {}) => {}, 0b{:b}",
			dir,
			index,
			result == 0,
			arr
		);
		result
	}

	unsafe fn can_process_sample_size(&self, symbolic_sample_size: i32) -> tresult {
		info!("can_process_sample_size({})", symbolic_sample_size);
		match symbolic_sample_size {
			K_SAMPLE32 => kResultTrue,
			K_SAMPLE64 => kResultFalse,
			_ => kInvalidArgument,
		}
	}

	unsafe fn get_latency_samples(&self) -> u32 {
		let dsp_ref = self.opus_dsp.borrow();

		match &*dsp_ref {
			Some(dsp) => {
				let frames = dsp.latency();
				info!("get_latency_samples() => {}", frames);
				frames as u32
			}
			None => {
				info!("get_latency_samples() => 0: not ready");
				0
			}
		}
	}

	unsafe fn setup_processing(&self, setup: *const ProcessSetup) -> tresult {
		let setup = &*setup;

		let mode = match setup.process_mode {
			0 => "realtime",
			1 => "prefetch",
			2 => "offline",
			x => {
				warn!("setup_processing() => {}: mode {}", kResultFalse, x);
				return kResultFalse;
			}
		};

		const OK: i32 = kResultTrue;
		match self.can_process_sample_size(setup.symbolic_sample_size) {
			OK => {}
			result => {
				warn!(
					"setup_processing() => {}: sample size {}",
					result, setup.symbolic_sample_size
				);
				return result;
			}
		}

		let dsp = match dsp::OpusDsp::new(setup) {
			Ok(dsp) => Some(Box::new(dsp)),
			Err(err) => {
				error!("setup_processing() => {}: {}", kInternalError, err);
				return kInternalError;
			}
		};

		// Save
		*self.opus_dsp.borrow_mut() = dsp;

		self.process_setup.borrow_mut().0 = *setup;

		info!(
			"setup_processing() {} f32 at {:.2} Hz with max {} per block ({:.2} ms)",
			mode,
			setup.sample_rate,
			setup.max_samples_per_block,
			1e3 * setup.max_samples_per_block as f64 / setup.sample_rate
		);

		kResultOk
	}

	///
	unsafe fn set_processing(&self, state: TBool) -> tresult {
		info!("set_processing({})", state);

		if state == 0 {
			let mut dsp = {
				let dsp_ref = graceful!(self.opus_dsp.try_borrow_mut());

				match *dsp_ref {
					Some(_) => RefMut::map(dsp_ref, |x| x.as_mut().unwrap().as_mut()),

					None => {
						error!("setup_processing() must be called first");
						return kNotInitialized;
					}
				}
			};

			graceful!(dsp.reset());
		}

		kResultTrue
	}

	///
	unsafe fn process(&self, data: *mut ProcessData) -> tresult {
		// Convert pointer to reference for borrow checking
		let data = &mut *data;

		// Verify that setup_processing() has not been called first, and unpack
		let mut dsp = {
			let dsp_ref = graceful!(self.opus_dsp.try_borrow_mut());

			match *dsp_ref {
				Some(_) => RefMut::map(dsp_ref, |x| x.as_mut().unwrap().as_mut()),

				None => {
					error!("setup_processing() must be called first");
					return kNotInitialized;
				}
			}
		};

		// TODO: Are these MIDI events???
		if let Some(input_events) = data.input_events.upgrade() {
			let num_events = input_events.get_event_count();
			if num_events > 0 {
				info!("process() NUM EVENTS {}", num_events);
			}
		}

		// Convert parameter queues to map type
		let input_params = dsp::upgrade_param_changes(&data.input_param_changes);

		// Apply parameters and return when there are no buses
		if data.num_inputs == 0 && data.num_outputs == 0 {
			graceful!(dsp.apply_parameter_changes(&input_params, usize::MAX));
			return kResultOk;
		}

		// Expect stereo in and stereo out
		let (in_bus, mut out_bus) = match dsp::try_stereo_buses(data) {
			Some(tuple) => tuple,
			None => {
				error!("process() stereo input and output buses are expected");
				error!("input buses: {}", data.num_inputs);
				error!("output buses: {}", data.num_outputs);
				return kInvalidArgument;
			}
		};

		// Perform frame-wise calculation of output and write to buffers
		graceful!(dsp.process(&in_bus, &mut out_bus, &input_params));

		kResultOk
	}

	///
	unsafe fn get_tail_samples(&self) -> u32 {
		info!("get_tail_samples()");
		0
	}
}
