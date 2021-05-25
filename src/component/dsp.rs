use super::params::Parameter;
use super::params::ParameterAs;
use anyhow::Result;
use audiopus::coder::Decoder;
use audiopus::coder::Encoder;
use audiopus::Application;
use audiopus::Channels;
use audiopus::SampleRate;
use enum_map::EnumMap;
use itertools::Itertools;
use log::*;
use samplerate::ConverterType;
use samplerate::Samplerate;
use std::convert::TryFrom;
use std::slice;
use std::{collections::VecDeque, iter::repeat};
use vst3_sys::vst::ProcessData;
use vst3_sys::vst::ProcessSetup;
use vst3_sys::{
	base::kResultTrue,
	utils::VstPtr,
	vst::{AudioBusBuffers, IParamValueQueue, IParameterChanges},
};

pub type ParamValueQueue = Option<Box<dyn IParamValueQueue>>;
pub type ParamQueueMap = EnumMap<Parameter, ParamValueQueue>;

pub unsafe fn upgrade_param_changes(ptr: &VstPtr<dyn IParameterChanges>) -> ParamQueueMap {
	let mut param_changes_map = EnumMap::<Parameter, ParamValueQueue>::default();

	if let Some(param_changes) = ptr.upgrade() {
		for i in 0..param_changes.get_parameter_count() {
			if let Some(param_queue) = param_changes.get_parameter_data(i).upgrade() {
				if let Ok(param) = Parameter::try_from(param_queue.get_parameter_id()) {
					if let Some(_) = param_changes_map[param] {
						warn!("duplicate parameter queue {:?}", param);
					} else {
						param_changes_map[param] = Some(Box::new(param_queue));
					}
				}
			}
		}
	}

	param_changes_map
}

pub unsafe fn try_stereo_buses(data: &mut ProcessData) -> Option<(StereoInput, StereoOutput)> {
	let num_samples = data.num_samples as usize;
	let in_buses = slice::from_raw_parts(data.inputs, data.num_inputs as usize);
	let out_buses = slice::from_raw_parts_mut(data.outputs, data.num_outputs as usize);
	let in_bus = StereoInput::from(in_buses.get(0)?, num_samples)?;
	let out_bus = StereoOutput::from(out_buses.get_mut(0)?, num_samples)?;
	Some((in_bus, out_bus))
}

pub struct StereoInput<'a> {
	inner: &'a AudioBusBuffers,
	c0: &'a [f32],
	c1: &'a [f32],
}

impl<'a> StereoInput<'a> {
	unsafe fn from(inner: &'a AudioBusBuffers, num_samples: usize) -> Option<Self> {
		let num_channels = inner.num_channels as usize;
		// De-pointer the list of channels
		let buffers = slice::from_raw_parts(inner.buffers as *const *const f32, num_channels);
		// De-pointer the first two channels from bus
		let c0 = slice::from_raw_parts(*buffers.get(0)?, num_samples);
		let c1 = slice::from_raw_parts(*buffers.get(1)?, num_samples);
		Some(Self { inner, c0, c1 })
	}
}

pub struct StereoOutput<'a> {
	inner: &'a mut AudioBusBuffers,
	c0: &'a mut [f32],
	c1: &'a mut [f32],
}

impl<'a> StereoOutput<'a> {
	unsafe fn from(inner: &'a mut AudioBusBuffers, num_samples: usize) -> Option<Self> {
		let num_channels = inner.num_channels as usize;
		// De-pointer the list of channels
		let buffers = slice::from_raw_parts(inner.buffers as *const *mut f32, num_channels);
		// De-pointer the first two channels from bus
		let c0 = slice::from_raw_parts_mut(*buffers.get(0)?, num_samples);
		let c1 = slice::from_raw_parts_mut(*buffers.get(1)?, num_samples);
		Some(Self { inner, c0, c1 })
	}
}

pub struct OpusDsp {
	latency: usize,
	silent: bool,
	queue_inner: VecDeque<f32>,
	queue_outer: VecDeque<f32>,
	converter_in: Samplerate,
	converter_out: Samplerate,
	pub bypass: bool,
	pub decode_fec: bool,
	pub decoder: Decoder,
	pub encoder: Encoder,
}

impl OpusDsp {
	const OPUS_SR: SampleRate = SampleRate::Hz48000;
	const OPUS_CH: Channels = Channels::Stereo;
	const OPUS_FRAMES: usize = Self::OPUS_SR as usize * 20 / 1000;
	const OPUS_INTERLEAVED: usize = Self::OPUS_FRAMES * Self::OPUS_CH as usize;

	///
	pub fn new(setup: &ProcessSetup) -> Result<Self> {
		let encoder = Encoder::new(Self::OPUS_SR, Self::OPUS_CH, Application::Voip)?;
		let decoder = Decoder::new(Self::OPUS_SR, Self::OPUS_CH)?;

		let inner_rate = Self::OPUS_SR as u32;
		let outer_rate = setup.sample_rate.round() as u32;

		let queue_inner = VecDeque::new();
		let queue_outer = VecDeque::new();

		let quality = ConverterType::Linear;
		let converter_in = Samplerate::new(quality, outer_rate, inner_rate, 2)?;
		let converter_out = Samplerate::new(quality, inner_rate, outer_rate, 2)?;

		let mut dsp = Self {
			latency: 0,
			silent: true,
			converter_in,
			converter_out,
			queue_inner,
			queue_outer,
			encoder,
			bypass: false,
			decode_fec: false,
			decoder,
		};

		dsp.reset()?;

		Ok(dsp)
	}

	///
	fn to_outer_frames(&self, inner_frames: usize) -> usize {
		let n = inner_frames * self.converter_in.from_rate() as usize;
		let d = Self::OPUS_SR as usize;
		n / d + usize::from(n % d != 0)
	}

	///
	pub fn reset(&mut self) -> Result<()> {
		let outer_sr = self.converter_in.from_rate() as f64;
		let inner_sr = self.converter_in.to_rate() as f64;

		self.converter_in.reset()?;
		self.converter_out.reset()?;
		self.queue_inner.clear();
		self.queue_outer.clear();

		let resample_a_latency = {
			let a_in = self.to_outer_frames(Self::OPUS_FRAMES);
			let data = self.converter_in.process(&vec![0.0; a_in])?;
			let a_out = Self::OPUS_FRAMES;
			let a_out_real = data.len();
			let a = a_out.saturating_sub(a_out_real);
			a
		};

		if resample_a_latency > 0 {
			warn!(
				"reset() resample_a_latency {} ({:.2} ms @ {} Hz)",
				resample_a_latency,
				1e3 * resample_a_latency as f64 / inner_sr,
				inner_sr
			);
		}

		let resample_b_latency = {
			let b_in = Self::OPUS_FRAMES;
			let data = self.converter_out.process(&vec![0.0; b_in])?;
			let b_out = self.to_outer_frames(Self::OPUS_FRAMES);
			let b_out_real = data.len();
			let b = b_out.saturating_sub(b_out_real);
			b
		};

		if resample_b_latency > 0 {
			warn!(
				"reset() resample_b_latency {} ({:.2} ms @ {} Hz)",
				resample_b_latency,
				1e3 * resample_b_latency as f64 / outer_sr,
				outer_sr
			);
		}

		self.latency = self.to_outer_frames(resample_a_latency)
			+ resample_b_latency
			+ self.to_outer_frames(Self::OPUS_FRAMES);

		Ok(())
	}

	///
	pub fn latency(&self) -> usize {
		self.latency
	}

	///
	pub fn process(
		&mut self,
		bus_in: &StereoInput,
		bus_out: &mut StereoOutput,
		params: &ParamQueueMap,
	) -> Result<()> {
		// Peform our Opus Paravulum effect on the audio stream!
		//
		// VSTs process audio in blocks. The Opus codec process audio in packets.
		// The lengths of blocks and packets are almost always different.
		// This means there will be partial, unprocess-able audio at the end.
		// A latency is choosen instead of small gaps of silence.

		let silent_prev = self.silent;
		self.silent = bus_in.inner.silence_flags & 0b11 != 0;

		// How many Opus packets should be processed?
		// In silence, empty the inner buffer and make one last packet.
		// In audio, process as many packets as possible.
		if self.silent {
			// Output one packet of audio if the last block had leftover audio.
			// This is okay since silence will (probably) last for multiple blocks.
			if !self.queue_inner.is_empty() {
				let mut data = self.queue_inner.drain(..).collect_vec();
				data.resize(Self::OPUS_INTERLEAVED, 0.0);
				self.process_packet(&mut data)?;
				let data = self.converter_out.process(&data)?;
				self.queue_outer.extend(data);
			}
		} else {
			// If the previous block was silent, assume the latency is drained.
			// Which means it needs to be replaced once audio starts again.
			if silent_prev {
				self.queue_inner.extend(vec![0.0; Self::OPUS_INTERLEAVED]);
			}

			let num_prev_inner = self.queue_inner.len();

			{
				let data = bus_in
					.c0
					.into_iter()
					.interleave(bus_in.c1)
					.copied()
					.collect_vec();

				self.queue_inner.extend(self.converter_in.process(&data)?);
			}

			let mut inner_frame_index = 0;
			while self.queue_inner.len() >= Self::OPUS_INTERLEAVED {
				// Determine current position in the new block of audio
				inner_frame_index += Self::OPUS_FRAMES;
				let inner_block_offset = inner_frame_index.saturating_sub(num_prev_inner);
				let block_offset = self.to_outer_frames(inner_block_offset);
				self.apply_parameter_changes(params, block_offset)?;

				// Process a packet
				let mut data = self
					.queue_inner
					.drain(..Self::OPUS_INTERLEAVED)
					.collect_vec();

				self.process_packet(&mut data)?;

				// Cache output for writing to bus
				let data = self.converter_out.process(&data)?;
				self.queue_outer.extend(data);
			}
		}

		if self.queue_outer.is_empty() {
			bus_out.inner.silence_flags = 0b11;
			bus_out.c0.fill(0.0);
			bus_out.c1.fill(0.0);
		} else {
			bus_out.inner.silence_flags = 0;
			let count = (bus_out.c0.len() + bus_out.c1.len()).min(self.queue_outer.len());
			let source = self.queue_outer.drain(..count).chain(repeat(0.0));
			let target = bus_out.c0.into_iter().interleave(bus_out.c1.into_iter());
			for (t, s) in target.zip(source) {
				*t = s
			}
		}

		self.apply_parameter_changes(params, usize::MAX)?;

		Ok(())
	}

	///
	fn process_packet(&mut self, signals: &mut [f32]) -> Result<()> {
		let mut packet_bytes = [0u8; 2048];
		// Encode
		let len = self.encoder.encode_float(signals, &mut packet_bytes)?;
		let packet = Some(&packet_bytes[..len]);
		// Decode
		let fec = self.encoder.inband_fec()?;
		self.decoder.decode_float(packet, &mut *signals, fec)?;
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

		for (param, option) in changes.iter() {
			if let Some(value) = option {
				match param {
					Parameter::Bypass => {
						self.bypass = value.as_bool();
					}
					Parameter::Complexity => {
						self.encoder.set_complexity(value.as_complexity())?;
					}
					Parameter::Gain => {
						self.decoder.set_gain(value.as_gain())?;
					}
					Parameter::InbandFec => {
						self.encoder.set_inband_fec(value.as_bool())?;
					}
					Parameter::MaxBandwith => {
						self.encoder.set_max_bandwidth(value.as_bandwidth())?;
					}
					Parameter::PredictedLoss => {
						self.encoder.set_packet_loss_perc(value.as_percentage())?;
					}
				}
			}
		}

		Ok(())
	}
}
