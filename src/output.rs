use std::sync::OnceLock;

static CONFIG: OnceLock<OutputConfig> = OnceLock::new();

pub struct OutputConfig {
    pub json: bool,
    pub raw: bool,
    pub quiet: bool,
}

pub fn init(json: bool, raw: bool, quiet: bool) {
    let _ = CONFIG.set(OutputConfig { json, raw, quiet });
}

fn get() -> &'static OutputConfig {
    CONFIG.get().expect("output::init() not called")
}

pub fn is_json() -> bool {
    get().json
}

pub fn is_raw() -> bool {
    get().raw
}

pub fn is_quiet() -> bool {
    get().quiet
}
