#[macro_export]
macro_rules! vst_result {
	($expr:expr) => {
		match $expr {
			Ok(x) => x,
			Err(err) => {
				error!("{}", err);
				return vst3_sys::base::kInternalError;
			}
		}
	};
}
