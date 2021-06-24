mod effect;
mod factory;
mod macros;
mod vst_str;

use log::*;
use simple_logger::SimpleLogger;
use vst3_com::c_void;

fn init() {
	SimpleLogger::new().init().unwrap();
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "system" fn GetPluginFactory() -> *mut c_void {
	info!("GetPluginFactory()");
	Box::into_raw(factory::Factory::new()) as *mut c_void
}

#[cfg(target_os = "linux")]
#[no_mangle]
pub extern "system" fn ModuleEntry(_: *mut c_void) -> bool {
	init();
	info!("ModuleEntry()");
	true
}

#[cfg(target_os = "linux")]
#[no_mangle]
pub extern "system" fn ModuleExit() -> bool {
	info!("ModuleExit()");
	true
}

#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "system" fn bundleEntry() -> bool {
	init();
	info!("bundleEntry()");
	true
}

#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "system" fn bundleExit() -> bool {
	info!("bundleExit()");
	true
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub extern "system" fn InitDll() -> bool {
	init();
	info!("InitDll()");
	true
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub extern "system" fn ExitDll() -> bool {
	info!("ExitDll()");
	true
}
