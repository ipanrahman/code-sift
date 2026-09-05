pub struct Config {
    pub verbose: bool,
    pub output: String,
}

impl Config {
    pub fn default() -> Self {
        Config {
            verbose: false,
            output: String::new(),
        }
    }

    pub fn load() -> Self {
        let mut cfg = Self::default();
        cfg.output = "out.txt".to_string();
        cfg
    }
}
