use crate::effect::OpusController;
use crate::effect::OpusProcessor;
use crate::effect::VstClassInfo;
use std::os::raw::c_void;
use vst3_com::IID;
use vst3_sys::base::IPluginFactory;
use vst3_sys::base::IPluginFactory2;
use vst3_sys::base::IPluginFactory3;
use vst3_sys::VST3;

#[VST3(implements(IPluginFactory, IPluginFactory2, IPluginFactory3))]
pub struct Factory {}

impl Factory {
	pub fn new() -> Box<Self> {
		Self::allocate()
	}

	pub const VENDOR_NAME: &'static str = "astra137";
	pub const VENDOR_EMAIL: &'static str = "maccelerated@gmail.com";
	pub const VENDOR_URL: &'static str = "https://github.com/astra137";
	pub const COMPONENT_VERSION: &'static str = env!("CARGO_PKG_VERSION");
	pub const COMPONENT_SDK_VERSION: &'static str = "VST 3.6.13";

	pub const CLASSES: i32 = 2;

	pub fn get_class(index: i32) -> Option<VstClassInfo> {
		match index {
			0 => Some(OpusProcessor::INFO),
			1 => Some(OpusController::INFO),
			_ => None,
		}
	}

	pub fn create_class(cid: &IID, _iid: &IID) -> Option<*mut c_void> {
		match *cid {
			OpusProcessor::CID => Some(OpusProcessor::create_instance()),
			OpusController::CID => Some(OpusController::create_instance()),
			_ => None,
		}
	}
}

mod vst {
	use super::Factory;
	use crate::effect::VstClassInfo;
	use crate::vst_str;
	use log::*;
	use vst3_com::c_void;
	use vst3_com::IID;
	use vst3_sys::base::FactoryFlags;
	use vst3_sys::base::IPluginFactory;
	use vst3_sys::base::IPluginFactory2;
	use vst3_sys::base::IPluginFactory3;
	use vst3_sys::base::PClassInfo;
	use vst3_sys::base::PClassInfo2;
	use vst3_sys::base::PClassInfoW;
	use vst3_sys::base::PFactoryInfo;
	use vst3_sys::base::{kInvalidArgument, kResultFalse, kResultOk, tresult};

	impl IPluginFactory for Factory {
		unsafe fn get_factory_info(&self, info: *mut PFactoryInfo) -> tresult {
			info!("get_factory_info()");

			(*info) = PFactoryInfo {
				vendor: vst_str::str_8(Self::VENDOR_NAME),
				url: vst_str::str_8(Self::VENDOR_URL),
				email: vst_str::str_8(Self::VENDOR_EMAIL),
				flags: FactoryFlags::kUnicode as i32,
			};

			kResultOk
		}

		unsafe fn count_classes(&self) -> i32 {
			info!("count_classes()");
			Self::CLASSES
		}

		unsafe fn get_class_info(&self, index: i32, info: *mut PClassInfo) -> tresult {
			info!("get_class_info()");

			match Self::get_class(index) {
				Some(VstClassInfo {
					cid,
					cardinality,
					category,
					name,
					..
				}) => {
					*info = PClassInfo {
						cid,
						cardinality,
						category: vst_str::str_8(category),
						name: vst_str::str_8(name),
					};
					kResultOk
				}

				None => {
					warn!("no such class: {}", index);
					kInvalidArgument
				}
			}
		}

		unsafe fn create_instance(
			&self,
			cid: *const IID,
			iid: *const IID,
			obj: *mut *mut c_void,
		) -> tresult {
			info!("create_instance({:?}, {:?})", *cid, *iid);

			match Self::create_class(&*cid, &*iid) {
				Some(ptr) => {
					*obj = ptr;
					kResultOk
				}

				None => {
					warn!("no such class: {:?}", cid);
					kInvalidArgument
				}
			}
		}
	}

	impl IPluginFactory2 for Factory {
		unsafe fn get_class_info2(&self, index: i32, info: *mut PClassInfo2) -> tresult {
			info!("get_class_info2({})", index);

			match Self::get_class(index) {
				Some(VstClassInfo {
					cid,
					cardinality,
					category,
					subcategories,
					class_flags,
					name,
				}) => {
					*info = PClassInfo2 {
						cid,
						cardinality,
						category: vst_str::str_8(category),
						subcategories: vst_str::str_8(subcategories),
						class_flags,
						name: vst_str::str_8(name),
						vendor: vst_str::str_8(Self::VENDOR_NAME),
						version: vst_str::str_8(Self::COMPONENT_VERSION),
						sdk_version: vst_str::str_8(Self::COMPONENT_SDK_VERSION),
					};
					kResultOk
				}

				None => {
					warn!("no such class: {}", index);
					kInvalidArgument
				}
			}
		}
	}

	impl IPluginFactory3 for Factory {
		unsafe fn get_class_info_unicode(&self, index: i32, info: *mut PClassInfoW) -> tresult {
			info!("get_class_info_unicode({})", index);

			match Self::get_class(index) {
				Some(VstClassInfo {
					cid,
					cardinality,
					category,
					subcategories,
					class_flags,
					name,
				}) => {
					*info = PClassInfoW {
						cid,
						cardinality,
						category: vst_str::str_8(category),
						subcategories: vst_str::str_8(subcategories),
						class_flags,
						name: vst_str::str_16(name),
						vendor: vst_str::str_16(Self::VENDOR_NAME),
						version: vst_str::str_16(Self::COMPONENT_VERSION),
						sdk_version: vst_str::str_16(Self::COMPONENT_SDK_VERSION),
					};
					kResultOk
				}

				None => {
					warn!("no such class: {}", index);
					kInvalidArgument
				}
			}
		}

		unsafe fn set_host_context(&self, _: *mut c_void) -> tresult {
			info!("set_host_context()");
			kResultFalse
		}
	}

	#[cfg(test)]
	mod tests {
		use super::Factory;
		use std::ffi::CStr;
		use std::mem::MaybeUninit;
		use vst3_sys::base::IPluginFactory;
		use vst3_sys::base::IPluginFactory2;
		use vst3_sys::base::IPluginFactory3;
		use vst3_sys::base::PClassInfo;
		use vst3_sys::base::PClassInfo2;
		use vst3_sys::base::PClassInfoW;

		#[test]
		fn const_component_sdk_version() {
			let c_str = unsafe { CStr::from_ptr(vst3_sys::vst::kVstVersionString) };
			assert_eq!(Factory::COMPONENT_SDK_VERSION, c_str.to_str().unwrap());
		}

		#[test]
		fn component_infos_dont_panic() {
			let mut a = unsafe { MaybeUninit::zeroed().assume_init() };
			let mut b = unsafe { MaybeUninit::zeroed().assume_init() };
			let mut c = unsafe { MaybeUninit::zeroed().assume_init() };

			let f = Factory::new();

			unsafe {
				assert_eq!(0, f.get_class_info(0, &mut a as *mut PClassInfo));
				assert_eq!(0, f.get_class_info2(0, &mut b as *mut PClassInfo2));
				assert_eq!(0, f.get_class_info_unicode(0, &mut c as *mut PClassInfoW));
			}

			unsafe {
				assert_eq!(0, f.get_class_info(1, &mut a as *mut PClassInfo));
				assert_eq!(0, f.get_class_info2(1, &mut b as *mut PClassInfo2));
				assert_eq!(0, f.get_class_info_unicode(1, &mut c as *mut PClassInfoW));
			}
		}
	}
}
