use crate::vst_str;
use anyhow::Result;
use audiopus::Bandwidth;
use enum_map::Enum;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use std::convert::Into;
use variant_count::VariantCount;
use vst3_sys::vst;
use vst3_sys::vst::ParameterFlags;
use vst3_sys::vst::ParameterInfo;
use vst3_sys::vst::UnitInfo;
use super::dsp::OpusDSP;

pub fn bandwidth_from_value(value: f64) -> Bandwidth {
	match (value * 4.0 + 0.5) as usize {
		0 => Bandwidth::Narrowband,
		1 => Bandwidth::Mediumband,
		2 => Bandwidth::Wideband,
		3 => Bandwidth::Superwideband,
		4 => Bandwidth::Fullband,
		_ => Bandwidth::Auto,
	}
}

///
#[derive(Copy, Clone, Debug, Enum, IntoPrimitive, TryFromPrimitive, VariantCount)]
#[repr(i32)]
pub enum Unit {
	Root = vst::kRootUnitId,
	Encoder,
	Decoder,
	Network,
}

impl Unit {
	pub fn get_info(self) -> UnitInfo {
		match self {
			Self::Root => UnitInfo {
				id: self.into(),
				parent_unit_id: vst::kNoParentUnitId,
				name: vst_str::str_16("Root"),
				program_list_id: vst::kNoProgramListId,
			},
			Self::Encoder => UnitInfo {
				id: self.into(),
				parent_unit_id: Unit::Root.into(),
				name: vst_str::str_16("Encoder"),
				program_list_id: vst::kNoProgramListId,
			},
			Self::Decoder => UnitInfo {
				id: self.into(),
				parent_unit_id: Unit::Root.into(),
				name: vst_str::str_16("Decoder"),
				program_list_id: vst::kNoProgramListId,
			},
			Self::Network => UnitInfo {
				id: self.into(),
				parent_unit_id: Unit::Root.into(),
				name: vst_str::str_16("Network"),
				program_list_id: vst::kNoProgramListId,
			},
		}
	}
}

///
#[derive(Copy, Clone, Debug, Enum, IntoPrimitive, TryFromPrimitive, VariantCount)]
#[repr(u32)]
pub enum Parameter {
	Bypass,
	MaxBandwith,
	Complexity,
	PredictedLoss,
	RandomLoss,
	RoundRobinLoss,
}

impl Parameter {
	pub fn get_from_dsp(self, dsp: &OpusDSP) -> Result<f64> {
		let value = match self {
			Self::Bypass => dsp.bypass as u8 as f64,
			Self::RandomLoss => dsp.loss_random,
			Self::RoundRobinLoss => dsp.loss_roundrobin,
			Self::PredictedLoss => f64::from(dsp.encoder.packet_loss_perc()?) / 100.0,
			Self::Complexity => f64::from(dsp.encoder.complexity()?) / 10.0,
			Self::MaxBandwith => match dsp.encoder.max_bandwidth()? {
				Bandwidth::Narrowband => 0.0,
				Bandwidth::Mediumband => 0.25,
				Bandwidth::Wideband => 0.5,
				Bandwidth::Superwideband => 0.75,
				Bandwidth::Fullband => 1.0,
				Bandwidth::Auto => 1.0,
			},
		};

		Ok(value)
	}

	pub fn set_to_dsp(self, dsp: &mut OpusDSP, value: f64) -> Result<()> {
		match self {
			Parameter::Bypass => dsp.bypass = value > 0.5,
			Parameter::RandomLoss => dsp.loss_random = value,
			Parameter::RoundRobinLoss => dsp.loss_roundrobin = value,
			Parameter::PredictedLoss => {
				let percentage = (value * 100.0 + f64::EPSILON) as u8;
				dsp.encoder.set_packet_loss_perc(percentage)?
			}
			Parameter::Complexity => {
				let complexity = (value * 10.0 + f64::EPSILON) as u8;
				dsp.encoder.set_complexity(complexity)?
			}
			Parameter::MaxBandwith => {
				let bw = match (value * 4.0 + f64::EPSILON) as usize {
					0 => Bandwidth::Narrowband,
					1 => Bandwidth::Mediumband,
					2 => Bandwidth::Wideband,
					3 => Bandwidth::Superwideband,
					4 => Bandwidth::Fullband,
					_ => Bandwidth::Auto,
				};
				dsp.encoder.set_max_bandwidth(bw)?
			}
		};

		Ok(())
	}

	pub fn get_parameter_info(self) -> ParameterInfo {
		match self {
			Self::Bypass => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Bypass"),
				short_title: [0; 128],
				units: [0; 128],
				step_count: 1,
				default_normalized_value: 0.0,
				unit_id: Unit::Root.into(),
				flags: ParameterFlags::kCanAutomate as i32 | ParameterFlags::kIsBypass as i32,
			},

			Self::MaxBandwith => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Max Bandwith"),
				short_title: vst_str::str_16("Band"),
				units: vst_str::str_16("kHz"),
				step_count: 5 - 1,
				default_normalized_value: 1.0,
				unit_id: Unit::Encoder.into(),
				flags: ParameterFlags::kCanAutomate as i32,
			},

			Self::Complexity => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Complexity"),
				short_title: vst_str::str_16("Cmpx"),
				units: vst_str::str_16(""),
				step_count: 10,
				default_normalized_value: 0.9,
				unit_id: Unit::Encoder.into(),
				flags: ParameterFlags::kCanAutomate as i32,
			},

			Self::PredictedLoss => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Predicted Loss"),
				short_title: vst_str::str_16("PdLs"),
				units: vst_str::str_16("%"),
				step_count: 100,
				default_normalized_value: 0.0,
				unit_id: Unit::Encoder.into(),
				flags: ParameterFlags::kCanAutomate as i32,
			},

			Self::RandomLoss => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Random Loss"),
				short_title: vst_str::str_16("RndLs"),
				units: vst_str::str_16("%"),
				step_count: 0,
				default_normalized_value: 0.0,
				unit_id: Unit::Network.into(),
				flags: ParameterFlags::kCanAutomate as i32,
			},

			Self::RoundRobinLoss => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Round Robin Loss"),
				short_title: vst_str::str_16("RRLs"),
				units: vst_str::str_16("%"),
				step_count: 0,
				default_normalized_value: 0.0,
				unit_id: Unit::Network.into(),
				flags: ParameterFlags::kCanAutomate as i32,
			},
		}
	}

	pub fn get_param_string_by_value(&self, value: f64) -> Option<String> {
		match self {
			Self::Bypass => None,
			Self::Complexity => Some(format!("{:.0}", value * 10.0)),
			Self::PredictedLoss => Some(format!("{:.0}", value * 100.0)),
			Self::RandomLoss => Some(format!("{:.2}", value * 100.0)),
			Self::RoundRobinLoss => Some(format!("{:.2}", value * 100.0)),
			Self::MaxBandwith => Some(
				match bandwidth_from_value(value) {
					Bandwidth::Narrowband => "4",
					Bandwidth::Mediumband => "6",
					Bandwidth::Wideband => "8",
					Bandwidth::Superwideband => "12",
					Bandwidth::Fullband => "20",
					Bandwidth::Auto => "Auto",
				}
				.to_string(),
			),
		}
	}

	pub fn get_param_value_by_string(&self, _string: &str) -> Option<f64> {
		match self {
			Self::Bypass => None,
			Self::PredictedLoss => None,
			Self::Complexity => None,
			Self::MaxBandwith => None,
			Self::RandomLoss => None,
			Self::RoundRobinLoss => None,
		}
	}

	pub fn normalized_param_to_plain(&self, value: f64) -> f64 {
		match self {
			Self::Bypass => value,
			Self::PredictedLoss => value,
			Self::Complexity => value,
			Self::MaxBandwith => value,
			Self::RandomLoss => value,
			Self::RoundRobinLoss => value,
		}
	}

	pub fn plain_param_to_normalized(&self, plain_value: f64) -> f64 {
		match self {
			Self::Bypass => plain_value,
			Self::PredictedLoss => plain_value,
			Self::Complexity => plain_value,
			Self::MaxBandwith => plain_value,
			Self::RandomLoss => plain_value,
			Self::RoundRobinLoss => plain_value,
		}
	}
}
