mod component;
mod vst_str;

use log::*;
use simple_logger::SimpleLogger;
use std::os::raw::c_void;
use vst3_com::IID;
use vst3_sys::base::IPluginFactory3;
use vst3_sys::base::{
	kInvalidArgument, kResultFalse, kResultOk, tresult, FactoryFlags, IPluginFactory,
	IPluginFactory2, PClassInfo, PClassInfo2, PClassInfoW, PFactoryInfo,
};
use vst3_sys::sys::GUID;
use vst3_sys::VST3;

pub const VENDOR_NAME: &str = "astra137";
pub const VENDOR_URL: &str = "https://github.com/astra137";
pub const VENDOR_EMAIL: &str = "maccelerated@gmail.com";

pub const COMPONENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMPONENT_SDK_VERSION: &str = "VST 3.6.13";

pub trait Component {
	const CID: GUID;
	const CARDINALITY: i32;
	const CATEGORY: &'static str;
	const NAME: &'static str;
	const CLASS_FLAGS: u32;
	const SUBCATEGORIES: &'static str;

	fn info() -> PClassInfo {
		PClassInfo {
			cid: Self::CID,
			cardinality: Self::CARDINALITY,
			category: vst_str::str_8(Self::CATEGORY),
			name: vst_str::str_8(Self::NAME),
		}
	}

	fn info2() -> PClassInfo2 {
		PClassInfo2 {
			cid: Self::CID,
			cardinality: Self::CARDINALITY,
			category: vst_str::str_8(Self::CATEGORY),
			name: vst_str::str_8(Self::NAME),
			class_flags: Self::CLASS_FLAGS,
			subcategories: vst_str::str_8(Self::SUBCATEGORIES),
			vendor: vst_str::str_8(VENDOR_NAME),
			version: vst_str::str_8(COMPONENT_VERSION),
			sdk_version: vst_str::str_8(COMPONENT_SDK_VERSION),
		}
	}

	fn info_w() -> PClassInfoW {
		PClassInfoW {
			cid: Self::CID,
			cardinality: Self::CARDINALITY,
			category: vst_str::str_8(Self::CATEGORY),
			name: vst_str::str_16(Self::NAME),
			class_flags: Self::CLASS_FLAGS,
			subcategories: vst_str::str_8(Self::SUBCATEGORIES),
			vendor: vst_str::str_16(VENDOR_NAME),
			version: vst_str::str_16(COMPONENT_VERSION),
			sdk_version: vst_str::str_16(COMPONENT_SDK_VERSION),
		}
	}
}

#[VST3(implements(IPluginFactory, IPluginFactory2, IPluginFactory3))]
pub struct Factory {}

impl Factory {
	fn new() -> Box<Self> {
		Self::allocate()
	}

	pub fn create_instance() -> *mut c_void {
		info!("create_instance()");
		Box::into_raw(Self::new()) as *mut c_void
	}
}

impl IPluginFactory3 for Factory {
	unsafe fn get_class_info_unicode(&self, index: i32, info: *mut PClassInfoW) -> tresult {
		info!("get_class_info_unicode({})", index);

		(*info) = match index {
			0 => component::OpusProcessor::info_w(),
			1 => component::OpusController::info_w(),
			_ => {
				warn!("Invalid class info ID {}", index);
				return kInvalidArgument;
			}
		};

		kResultOk
	}

	unsafe fn set_host_context(&self, _: *mut c_void) -> tresult {
		info!("set_host_context()");
		kResultFalse
	}
}

impl IPluginFactory2 for Factory {
	unsafe fn get_class_info2(&self, index: i32, info: *mut PClassInfo2) -> tresult {
		info!("get_class_info2({})", index);

		(*info) = match index {
			0 => component::OpusProcessor::info2(),
			1 => component::OpusController::info2(),
			_ => {
				warn!("Invalid class info ID {}", index);
				return kInvalidArgument;
			}
		};

		kResultOk
	}
}

impl IPluginFactory for Factory {
	unsafe fn get_factory_info(&self, info: *mut PFactoryInfo) -> tresult {
		info!("get_factory_info()");

		(*info) = PFactoryInfo {
			vendor: vst_str::str_8(VENDOR_NAME),
			url: vst_str::str_8(VENDOR_URL),
			email: vst_str::str_8(VENDOR_EMAIL),
			flags: FactoryFlags::kUnicode as i32,
		};

		kResultOk
	}

	unsafe fn count_classes(&self) -> i32 {
		info!("count_classes()");
		2
	}

	unsafe fn get_class_info(&self, index: i32, info: *mut PClassInfo) -> tresult {
		info!("get_class_info()");

		*info = match index {
			0 => component::OpusProcessor::info(),
			1 => component::OpusController::info(),
			_ => {
				warn!("Invalid class info ID {}", index);
				return kInvalidArgument;
			}
		};

		kResultOk
	}

	unsafe fn create_instance(
		&self,
		cid: *const IID,
		_iid: *const IID,
		obj: *mut *mut c_void,
	) -> tresult {
		info!("Query _iid: {:?}", *_iid);
		info!("Creating instance of {:?}", *cid);

		match *cid {
			component::OpusProcessor::CID => {
				*obj = component::OpusProcessor::create_instance();
				kResultOk
			}

			component::OpusController::CID => {
				*obj = component::OpusController::create_instance();
				kResultOk
			}

			unknown_cid => {
				warn!("CID not found: {:?}", unknown_cid);
				kResultFalse
			}
		}
	}
}

//////////////////////////////////////////////////////////////////////////////////////////////////

fn init() {
	SimpleLogger::new().init().unwrap();
}

#[no_mangle]
#[allow(non_snake_case)]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "system" fn GetPluginFactory() -> *mut c_void {
	info!("GetPluginFactory()");
	Factory::create_instance()
}

#[cfg(target_os = "linux")]
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn ModuleEntry(_: *mut c_void) -> bool {
	init();
	info!("ModuleEntry()");
	true
}

#[cfg(target_os = "linux")]
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn ModuleExit() -> bool {
	info!("ModuleExit()");
	true
}

#[cfg(target_os = "macos")]
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn bundleEntry() -> bool {
	init();
	info!("bundleEntry()");
	true
}

#[cfg(target_os = "macos")]
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn bundleExit() -> bool {
	info!("bundleExit()");
	true
}

#[cfg(target_os = "windows")]
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn InitDll() -> bool {
	init();
	info!("InitDll()");
	true
}

#[cfg(target_os = "windows")]
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn ExitDll() -> bool {
	info!("ExitDll()");
	true
}

//////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
	use crate::Component;
	use std::ffi::CStr;

	#[test]
	fn const_component_sdk_version() {
		let c_str = unsafe { CStr::from_ptr(vst3_sys::vst::kVstVersionString) };
		assert_eq!(super::COMPONENT_SDK_VERSION, c_str.to_str().unwrap());
	}

	#[test]
	fn component_infos_dont_panic() {
		super::component::OpusProcessor::info();
		super::component::OpusProcessor::info2();
		super::component::OpusProcessor::info_w();

		super::component::OpusController::info();
		super::component::OpusController::info2();
		super::component::OpusController::info_w();
	}
}
