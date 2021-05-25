use super::params::Parameter;
use super::params::ParameterFrom;
use super::ContextPtr;
use super::SaveState;
use crate::vst_str;
use crate::Component;
use enum_map::EnumMap;
use hex_literal::hex;
use log::*;
use std::cell::RefCell;
use std::mem::size_of;
use std::os::raw::c_void;
use std::ptr::null_mut;
use vst3_com::sys::GUID;
use vst3_com::ComPtr;
use vst3_sys::base::kInvalidArgument;
use vst3_sys::base::{
	kResultFalse, kResultOk, kResultTrue, tresult, ClassCardinality, FIDString, IBStream,
	IPluginBase, IUnknown,
};
use vst3_sys::utils::VstPtr;
use vst3_sys::vst::String128;
use vst3_sys::vst::{
	IComponentHandler, IEditController, IUnitInfo, ParameterInfo, ProgramListInfo, TChar, UnitInfo,
};
use vst3_sys::VST3;

struct ComponentHandler(*mut c_void);

#[VST3(implements(IEditController, IUnitInfo))]
pub struct OpusController {
	context: RefCell<ContextPtr>,
	component_handler: RefCell<ComponentHandler>,
	parameters: RefCell<EnumMap<Parameter, f64>>,
}

impl Component for OpusController {
	const CID: GUID = GUID {
		data: hex!("6aa2e096dae547c5ba690605fd776699"),
	};

	const CARDINALITY: i32 = ClassCardinality::kManyInstances as i32;
	const CATEGORY: &'static str = "Component Controller Class";
	const NAME: &'static str = "Opus Parvulum Controller";
	const CLASS_FLAGS: u32 = 0;
	const SUBCATEGORIES: &'static str = "";
}

impl OpusController {
	pub fn new() -> Box<Self> {
		let context = RefCell::new(ContextPtr(null_mut()));
		let component_handler = RefCell::new(ComponentHandler(null_mut()));
		let parameters = RefCell::new(EnumMap::default());
		OpusController::allocate(context, component_handler, parameters)
	}

	pub fn create_instance() -> *mut c_void {
		Box::into_raw(Self::new()) as *mut c_void
	}

	fn nth_parameter(&self, id: usize) -> Option<Parameter> {
		Some(self.parameters.borrow().iter().nth(id as usize)?.0)
	}
}

impl IEditController for OpusController {
	unsafe fn set_component_state(&self, state: *mut c_void) -> tresult {
		info!("set_component_state()");

		if state.is_null() {
			return kResultFalse;
		}

		let state = state as *mut *mut _;
		let state: ComPtr<dyn IBStream> = ComPtr::new(state);

		let mut save_state = SaveState::new();

		let mut num_bytes_read = 0;
		let saved_params_ptr = &mut save_state as *mut SaveState as *mut c_void;
		state.read(
			saved_params_ptr,
			size_of::<SaveState>() as i32,
			&mut num_bytes_read,
		);

		self.parameters
			.borrow_mut()
			.iter_mut()
			.for_each(|(param, val)| match param {
				Parameter::Bypass => val.from_bool(save_state.bypass),
				Parameter::InbandFec => val.from_bool(save_state.inband_fec),
				Parameter::PredictedLoss => val.from_percentage(save_state.packet_loss_perc),
				Parameter::Complexity => val.from_complexity(save_state.complexity),
				Parameter::Gain => val.from_gain(save_state.gain),
				Parameter::MaxBandwith => val.from_bandwidth(save_state.max_bandwidth),
			});

		kResultOk
	}

	unsafe fn set_state(&self, _state: *mut c_void) -> tresult {
		info!("set_state()");
		kResultOk
	}

	unsafe fn get_state(&self, _state: *mut c_void) -> tresult {
		info!("get_state()");
		kResultOk
	}

	unsafe fn get_parameter_count(&self) -> i32 {
		info!("get_parameter_count()");
		self.parameters.borrow().len() as i32
	}

	unsafe fn get_parameter_info(&self, id: i32, info: *mut ParameterInfo) -> tresult {
		if let Some(param) = self.nth_parameter(id as usize) {
			info!("get_parameter_info({}) => {:?}", id, param);
			*info = param.get_parameter_info();
			kResultTrue
		} else {
			error!("get_parameter_info({}) no such param", id);
			kResultFalse
		}
	}

	unsafe fn get_param_string_by_value(&self, id: u32, value: f64, string: *mut TChar) -> tresult {
		info!("get_param_string_by_value({},{:.2})", id, value);

		// Borrow pointer as String128, because that's the actual type in the SDK
		let string = &mut *(string as *mut String128);

		match self.nth_parameter(id as usize) {
			Some(param) => match param.get_param_string_by_value(value) {
				Some(new_string) => {
					*string = vst_str::str_16(&new_string);
					kResultTrue
				}
				None => kResultFalse,
			},
			None => kInvalidArgument,
		}
	}

	unsafe fn get_param_value_by_string(
		&self,
		id: u32,
		ptr: *const TChar,
		value: *mut f64,
	) -> tresult {
		// Copy the UTF-16 C string to Rust's string type
		// to isolate the rest of the codebase from FFI and unsafe code
		let string = vst_str::wcstr_to_str(ptr);

		info!("get_param_value_by_string({}, {})", id, string);

		match self.nth_parameter(id as usize) {
			Some(param) => match param.get_param_value_by_string(&string) {
				Some(new_value) => {
					*value = new_value;
					kResultTrue
				}
				None => kResultFalse,
			},
			None => kInvalidArgument,
		}
	}

	unsafe fn normalized_param_to_plain(&self, id: u32, value_normalized: f64) -> f64 {
		if let Some(param) = self.nth_parameter(id as usize) {
			param.normalized_param_to_plain(value_normalized)
		} else {
			value_normalized
		}
	}

	unsafe fn plain_param_to_normalized(&self, id: u32, plain_value: f64) -> f64 {
		if let Some(param) = self.nth_parameter(id as usize) {
			param.plain_param_to_normalized(plain_value)
		} else {
			plain_value
		}
	}

	unsafe fn get_param_normalized(&self, id: u32) -> f64 {
		if let Some(param) = self.nth_parameter(id as usize) {
			self.parameters.borrow()[param]
		} else {
			0.0
		}
	}

	unsafe fn set_param_normalized(&self, id: u32, value: f64) -> tresult {
		info!("set_param_normalized()");
		if let Some(param) = self.nth_parameter(id as usize) {
			self.parameters.borrow_mut()[param] = value;
			kResultTrue
		} else {
			kResultFalse
		}
	}

	unsafe fn set_component_handler(&self, handler: *mut c_void) -> tresult {
		info!("set_component_handler()");

		if self.component_handler.borrow().0 == handler {
			return kResultTrue;
		}

		if !self.component_handler.borrow().0.is_null() {
			let component_handler = self.component_handler.borrow_mut().0 as *mut *mut _;
			let component_handler: ComPtr<dyn IComponentHandler> = ComPtr::new(component_handler);
			component_handler.release();
		}

		self.component_handler.borrow_mut().0 = handler;
		if !self.component_handler.borrow().0.is_null() {
			let component_handler = self.component_handler.borrow_mut().0 as *mut *mut _;
			let component_handler: ComPtr<dyn IComponentHandler> = ComPtr::new(component_handler);
			component_handler.add_ref();
		}

		kResultTrue
	}

	unsafe fn create_view(&self, _name: FIDString) -> *mut c_void {
		info!("create_view()");
		null_mut()
	}
}

impl IPluginBase for OpusController {
	unsafe fn initialize(&self, context: *mut c_void) -> tresult {
		info!("initialize()");

		if !self.context.borrow().0.is_null() {
			return kResultFalse;
		}
		self.context.borrow_mut().0 = context;

		kResultOk
	}

	unsafe fn terminate(&self) -> tresult {
		info!("terminate()");

		if !self.component_handler.borrow().0.is_null() {
			let component_handler = self.component_handler.borrow_mut().0 as *mut *mut _;
			let component_handler: ComPtr<dyn IComponentHandler> = ComPtr::new(component_handler);
			component_handler.release();
			self.component_handler.borrow_mut().0 = null_mut();
		}
		self.context.borrow_mut().0 = null_mut();

		kResultOk
	}
}

impl IUnitInfo for OpusController {
	unsafe fn get_unit_count(&self) -> i32 {
		info!("get_unit_count()");
		2
	}

	unsafe fn get_unit_info(&self, unit_index: i32, info: *mut UnitInfo) -> tresult {
		(*info) = match unit_index {
			0 => UnitInfo {
				id: 1,
				parent_unit_id: 0,
				name: vst_str::str_16("Encoder"),
				program_list_id: -1,
			},
			1 => UnitInfo {
				id: 2,
				parent_unit_id: 0,
				name: vst_str::str_16("Decoder"),
				program_list_id: -1,
			},
			_ => {
				return kResultFalse;
			}
		};

		kResultTrue
	}

	unsafe fn get_program_list_count(&self) -> i32 {
		info!("get_program_list_count()");
		0
	}

	unsafe fn get_program_list_info(&self, _list_index: i32, _info: *mut ProgramListInfo) -> i32 {
		info!("get_program_list_info()");
		kResultFalse
	}

	unsafe fn get_program_name(&self, _list_id: i32, _program_index: i32, _name: *mut u16) -> i32 {
		info!("get_program_name()");
		kResultFalse
	}

	unsafe fn get_program_info(
		&self,
		_list_id: i32,
		_program_index: i32,
		_attribute_id: *const u8,
		_attribute_value: *mut u16,
	) -> i32 {
		info!("get_program_info()");
		kResultFalse
	}

	unsafe fn has_program_pitch_names(&self, _id: i32, _index: i32) -> i32 {
		info!("has_program_pitch_names()");
		kResultFalse
	}

	unsafe fn get_program_pitch_name(
		&self,
		_id: i32,
		_index: i32,
		_pitch: i16,
		_name: *mut u16,
	) -> i32 {
		info!("get_program_pitch_name()");
		kResultFalse
	}

	unsafe fn get_selected_unit(&self) -> i32 {
		info!("get_selected_unit()");
		0
	}

	unsafe fn select_unit(&self, _id: i32) -> i32 {
		info!("select_unit()");
		kResultFalse
	}

	unsafe fn get_unit_by_bus(
		&self,
		_type_: i32,
		_dir: i32,
		_index: i32,
		_channel: i32,
		_unit_id: *mut i32,
	) -> i32 {
		info!("set_unit_by_bus()");
		kResultFalse
	}

	unsafe fn set_unit_program_data(
		&self,
		_list_or_unit: i32,
		_program_index: i32,
		_data: VstPtr<dyn IBStream>,
	) -> i32 {
		info!("set_unit_program_data()");
		kResultFalse
	}
}
