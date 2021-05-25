use crate::vst_str;
use audiopus::Bandwidth;
use enum_map::Enum;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use std::convert::Into;
use vst3_sys::vst::ParameterFlags;
use vst3_sys::vst::ParameterInfo;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Enum, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum Parameter {
	// Unit 0
	Bypass,
	// Unit 1 "Encoder"
	Complexity,
	InbandFec,
	PredictedLoss,
	MaxBandwith,
	// Unit 2 "Decoder"
	Gain,
}

impl Parameter {
	pub fn get_parameter_info(self) -> ParameterInfo {
		match self {
			Self::Bypass => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Bypass"),
				short_title: [0; 128],
				units: [0; 128],
				step_count: 1,
				default_normalized_value: 0.0,
				unit_id: 0,
				flags: ParameterFlags::kCanAutomate as i32 | ParameterFlags::kIsBypass as i32,
			},

			Self::InbandFec => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Inband FEC"),
				short_title: vst_str::str_16("FEC"),
				units: vst_str::str_16(""),
				step_count: 1,
				default_normalized_value: 0.0,
				unit_id: 1,
				flags: ParameterFlags::kCanAutomate as i32,
			},

			Self::PredictedLoss => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Predicted Loss"),
				short_title: vst_str::str_16("Loss"),
				units: vst_str::str_16("%"),
				step_count: 100 - 0,
				default_normalized_value: 0.0,
				unit_id: 1,
				flags: ParameterFlags::kCanAutomate as i32,
			},

			Self::Complexity => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Complexity"),
				short_title: vst_str::str_16("Cmpx"),
				units: vst_str::str_16(""),
				step_count: 10 - 0,
				default_normalized_value: 0.9,
				unit_id: 1,
				flags: ParameterFlags::kCanAutomate as i32,
			},

			Self::MaxBandwith => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Max Bandwith"),
				short_title: vst_str::str_16("Band"),
				units: vst_str::str_16("kHz"),
				step_count: 5 - 1,
				default_normalized_value: 1.0,
				unit_id: 1,
				flags: ParameterFlags::kCanAutomate as i32,
			},

			Self::Gain => ParameterInfo {
				id: self.into(),
				title: vst_str::str_16("Gain"),
				short_title: vst_str::str_16("Gain"),
				units: vst_str::str_16("dB"),
				step_count: 16 - 1,
				default_normalized_value: 0.5,
				unit_id: 2,
				flags: ParameterFlags::kCanAutomate as i32,
			},
		}
	}

	pub fn get_param_string_by_value(&self, value: f64) -> Option<String> {
		match self {
			Self::Bypass => None,
			Self::InbandFec => None,
			Self::PredictedLoss => Some(format!("{}", value.as_percentage())),
			Self::Complexity => Some(format!("{}", value.as_complexity())),
			Self::MaxBandwith => match value.as_bandwidth() {
				Bandwidth::Narrowband => Some("4".to_string()),
				Bandwidth::Mediumband => Some("6".to_string()),
				Bandwidth::Wideband => Some("8".to_string()),
				Bandwidth::Superwideband => Some("12".to_string()),
				Bandwidth::Fullband => Some("20".to_string()),
				Bandwidth::Auto => Some("Auto".to_string()),
			},
			Self::Gain => Some(format!("{}", value.as_gain())),
		}
	}

	pub fn get_param_value_by_string(&self, _string: &str) -> Option<f64> {
		match self {
			Self::Bypass => None,
			Self::InbandFec => None,
			Self::PredictedLoss => None,
			Self::Complexity => None,
			Self::MaxBandwith => None,
			Self::Gain => None,
		}
	}

	pub fn normalized_param_to_plain(&self, value: f64) -> f64 {
		match self {
			Self::Bypass => value,
			Self::InbandFec => value,
			Self::PredictedLoss => value,
			Self::Complexity => value,
			Self::MaxBandwith => value,
			Self::Gain => value,
		}
	}

	pub fn plain_param_to_normalized(&self, plain_value: f64) -> f64 {
		match self {
			Self::Bypass => plain_value,
			Self::InbandFec => plain_value,
			Self::PredictedLoss => plain_value,
			Self::Complexity => plain_value,
			Self::MaxBandwith => plain_value,
			Self::Gain => plain_value,
		}
	}
}

pub trait ParameterAs {
	fn as_bool(&self) -> bool;
	fn as_gain(&self) -> i32;
	fn as_complexity(&self) -> u8;
	fn as_percentage(&self) -> u8;
	fn as_bandwidth(&self) -> Bandwidth;
}

impl ParameterAs for f64 {
	fn as_bool(&self) -> bool {
		*self > 0.5
	}

	fn as_gain(&self) -> i32 {
		// lerp through i16 range using value
		let precast = (-8 as f64) * (1.0 - self) + (8 as f64) * self;
		// cast to i16 first to be extra sure about range bounds
		precast as i16 as i32
	}

	fn as_complexity(&self) -> u8 {
		(self * 10.0 + 0.5) as u8
	}

	fn as_percentage(&self) -> u8 {
		(self * 100.0 + 0.5) as u8
	}

	fn as_bandwidth(&self) -> Bandwidth {
		match (self * 4.0 + 0.5) as usize {
			0 => Bandwidth::Narrowband,
			1 => Bandwidth::Mediumband,
			2 => Bandwidth::Wideband,
			3 => Bandwidth::Superwideband,
			4 => Bandwidth::Fullband,
			_ => Bandwidth::Auto,
		}
	}
}

pub trait ParameterFrom {
	fn from_bool(&mut self, from: bool);
	fn from_gain(&mut self, from: i32);
	fn from_complexity(&mut self, from: u8);
	fn from_percentage(&mut self, from: u8);
	fn from_bandwidth(&mut self, from: Bandwidth);
}

impl ParameterFrom for f64 {
	fn from_bool(&mut self, from: bool) {
		*self = match from {
			true => 1.0,
			false => 0.0,
		}
	}
	fn from_gain(&mut self, from: i32) {
		let min: i32 = -8;
		let max: i32 = 8;
		*self = (from - min) as f64 / (max - min) as f64
	}
	fn from_complexity(&mut self, from: u8) {
		*self = from as f64 / 10.0
	}
	fn from_percentage(&mut self, from: u8) {
		*self = from as f64 / 100.0
	}
	fn from_bandwidth(&mut self, from: Bandwidth) {
		*self = match from {
			Bandwidth::Narrowband => 0.0,
			Bandwidth::Mediumband => 0.25,
			Bandwidth::Wideband => 0.5,
			Bandwidth::Superwideband => 0.75,
			Bandwidth::Fullband => 1.0,
			Bandwidth::Auto => 1.0,
		}
	}
}
