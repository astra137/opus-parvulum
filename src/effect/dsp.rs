use super::params::Parameter;
use anyhow::ensure;
use anyhow::Result;
use audiopus::coder::Decoder;
use audiopus::coder::Encoder;
use audiopus::Application;
use audiopus::Channels;
use audiopus::SampleRate;
use dasp::frame::Stereo;
use dasp::interpolate::linear::Linear;
use dasp::signal::interpolate::Converter;
use dasp::Frame;
use dasp::Signal;
use enum_map::EnumMap;
use log::*;
use rand::prelude::*;
use std::convert::TryFrom;
use std::slice;
use vst3_sys::vst::ProcessData;
use vst3_sys::vst::ProcessSetup;
use vst3_sys::{
	base::kResultTrue,
	utils::VstPtr,
	vst::{IParamValueQueue, IParameterChanges},
};

pub type ParamQueueMap = EnumMap<Parameter, Option<Box<dyn IParamValueQueue>>>;

pub unsafe fn upgrade_param_changes(ptr: &VstPtr<dyn IParameterChanges>) -> ParamQueueMap {
	let mut param_changes_map = ParamQueueMap::default();

	if let Some(param_changes) = ptr.upgrade() {
		// For each parameter change queue
		for i in 0..param_changes.get_parameter_count() {
			if let Some(param_queue) = param_changes.get_parameter_data(i).upgrade() {
				if let Ok(param) = Parameter::try_from(param_queue.get_parameter_id()) {
					// Shouldn't happen?
					if param_changes_map[param].is_some() {
						warn!("duplicate parameter queue {:?}", param);
					}

					param_changes_map[param] = Some(Box::new(param_queue));
				}
			}
		}
	}

	param_changes_map
}

mod buffer_signal {
	use dasp::frame::Stereo;
	use dasp::interpolate::linear::Linear;
	use dasp::signal::interpolate::Converter;
	use dasp::Frame;
	use dasp::Signal;
	use std::collections::VecDeque;

	pub struct BufferSignal<F: Frame>(VecDeque<F>);

	impl<F: Frame> BufferSignal<F> {
		pub fn push(&mut self, elem: F) {
			self.0.push_back(elem);
		}

		pub fn push_slice(&mut self, slice: &[F]) {
			self.0.extend(slice);
		}
	}

	impl<F: Frame> Signal for BufferSignal<F> {
		type Frame = F;

		fn next(&mut self) -> F {
			self.0.pop_front().unwrap_or(F::EQUILIBRIUM)
		}

		fn is_exhausted(&self) -> bool {
			self.0.is_empty()
		}
	}

	pub fn new(
		source_hz: f64,
		target_hz: f64,
	) -> Converter<BufferSignal<Stereo<f32>>, Linear<Stereo<f32>>> {
		let buffer = VecDeque::new();
		let interpolator = Linear::new(Stereo::EQUILIBRIUM, Stereo::EQUILIBRIUM);
		BufferSignal(buffer).from_hz_to_hz(interpolator, source_hz, target_hz)
	}
}

pub struct OpusDSP {
	sample_rate: f64,
	insignal: Converter<buffer_signal::BufferSignal<Stereo<f32>>, Linear<Stereo<f32>>>,
	outsignal: Converter<buffer_signal::BufferSignal<Stereo<f32>>, Linear<Stereo<f32>>>,
	rng: ThreadRng,
	pub bypass: bool,
	pub loss_roundrobin: f64,
	pub loss_random: f64,
	pub decoder: Decoder,
	pub encoder: Encoder,
}

const OPUS_SR: SampleRate = SampleRate::Hz48000;
const OPUS_SRF: f64 = OPUS_SR as i32 as f64;
const OPUS_LEN: usize = 960;

impl Default for OpusDSP {
	fn default() -> Self {
		Self::new()
	}
}

impl OpusDSP {
	///
	fn new() -> Self {
		let sample_rate = OPUS_SRF;
		let insignal = buffer_signal::new(sample_rate, OPUS_SRF);
		let outsignal = buffer_signal::new(OPUS_SRF, sample_rate);
		let encoder = Encoder::new(OPUS_SR, Channels::Stereo, Application::Voip).unwrap();
		let decoder = Decoder::new(OPUS_SR, Channels::Stereo).unwrap();

		Self {
			sample_rate,
			bypass: false,
			loss_roundrobin: 0.0,
			loss_random: 0.0,
			rng: thread_rng(),
			insignal,
			outsignal,
			encoder,
			decoder,
		}
	}

	///
	pub fn setup(&mut self, setup: &ProcessSetup) -> Result<()> {
		self.sample_rate = setup.sample_rate;
		self.encoder = Encoder::new(OPUS_SR, Channels::Stereo, Application::Voip)?;
		self.decoder = Decoder::new(OPUS_SR, Channels::Stereo)?;
		self.reset();
		Ok(())
	}

	///
	pub fn reset(&mut self) {
		self.insignal = buffer_signal::new(self.sample_rate, OPUS_SRF);
		self.outsignal = buffer_signal::new(OPUS_SRF, self.sample_rate);
	}

	///
	fn outer_frames(&self, inner_frames: usize) -> usize {
		(inner_frames as f64 * self.sample_rate / OPUS_SRF) as usize
	}

	///
	pub fn latency(&self) -> usize {
		self.outer_frames(OPUS_LEN)
	}

	///
	pub unsafe fn process(&mut self, data: &ProcessData) -> Result<()> {
		let num_samples = data.num_samples as usize;

		let (in_bus, in0, in1) = {
			let buses = slice::from_raw_parts(data.inputs, data.num_inputs as usize);
			ensure!(!buses.is_empty(), "requires at least 1 input bus");
			let bus = &buses[0];
			let num_channels = bus.num_channels as usize;
			let buffers = slice::from_raw_parts(bus.buffers as *const *const f32, num_channels);
			ensure!(buffers.len() >= 2, "requires at least 2 output channels");
			let c0 = slice::from_raw_parts(buffers[0], num_samples);
			let c1 = slice::from_raw_parts(buffers[1], num_samples);
			(bus, c0, c1)
		};

		let (out_bus, out0, out1) = {
			let buses = slice::from_raw_parts_mut(data.outputs, data.num_outputs as usize);
			ensure!(!buses.is_empty(), "requires at least 1 output bus");
			let bus = &mut buses[0];
			let num_channels = bus.num_channels as usize;
			let buffers = slice::from_raw_parts(bus.buffers as *const *mut f32, num_channels);
			ensure!(buffers.len() >= 2, "requires at least 2 output channels");
			let c0 = slice::from_raw_parts_mut(buffers[0], num_samples);
			let c1 = slice::from_raw_parts_mut(buffers[1], num_samples);
			(bus, c0, c1)
		};

		let params = upgrade_param_changes(&data.input_param_changes);

		let is_silent = in_bus.silence_flags & 0b11 == 0b11;

		if is_silent && self.insignal.is_exhausted() {
			// silence
			out_bus.silence_flags = 0b11;
			out0.fill(Stereo::EQUILIBRIUM[0]);
			out1.fill(Stereo::EQUILIBRIUM[1]);
		} else {
			// process
			for i in 0..num_samples {
				if self.outsignal.is_exhausted() {
					let mut packet_audio = [[0f32; 2]; OPUS_LEN];
					let mut packet_bytes = [0u8; 1024];

					// Read 1 packet of input
					packet_audio.fill_with(|| self.insignal.next());

					// Reslice
					let signals = dasp::slice::to_sample_slice_mut(&mut packet_audio[..]);

					// Apply params up to this frame
					self.apply_parameter_changes(&params, i)?;

					// Encode
					let len = self.encoder.encode_float(signals, &mut packet_bytes)?;
					let packet = Some(&packet_bytes[..len]);

					// Decode
					if self.rng.gen::<f64>() < self.loss_random {
						let lost: Option<&[u8]> = None;
						self.decoder.decode_float(lost, signals, true)?;
					} else {
						self.decoder.decode_float(packet, signals, false)?;
					}

					// Cache output
					self.outsignal.source_mut().push_slice(&packet_audio);
				}

				if !is_silent {
					self.insignal.source_mut().push([in0[i], in1[i]]);
				}

				let [s0, s1] = self.outsignal.next();
				out0[i] = s0;
				out1[i] = s1;
			}
		}

		self.apply_parameter_changes(&params, usize::MAX)?;

		Ok(())
	}

	///
	pub fn apply_parameter_changes(&mut self, map: &ParamQueueMap, limit: usize) -> Result<()> {
		let mut changes = EnumMap::<Parameter, Option<f64>>::default();

		for (param, option) in map.iter() {
			if let Some(queue) = option {
				let mut a = None;
				// let mut b = None;
				let num_points = unsafe { queue.get_point_count() };
				let mut offset = 0;
				let mut value = 0.0;
				for i in 0..num_points {
					let result = unsafe { queue.get_point(i, &mut offset, &mut value) };
					if result == kResultTrue {
						if (offset as usize) < limit {
							// Found next point within sample range
							a = Some(value);
						} else {
							// TODO Found point after allowed range, use as target for interpolation
							break;
						}
					}
				}
				changes[param] = a;
			}
		}

		for (param, value) in changes.iter() {
			if let Some(value) = value {
				param.set_to_dsp(self, *value)?;
			}
		}

		Ok(())
	}
}
