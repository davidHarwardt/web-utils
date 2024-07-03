use std::{env, path::{Path, PathBuf}, process::Command};

pub use serde_json::json;

impl Default for BuildConfig { fn default() -> Self { Self::new() } }
/// config for building tailwind
///
/// ```rust
/// // example
/// BuildConfig::new().with_cdn_src("https://my.cdn.com").build()?;
/// ```
#[derive(Debug, Clone)]
pub struct BuildConfig {
    css_path: Option<PathBuf>,
    always: bool,
    tailwind_config: serde_json::Value,
    cdn_src: String,
}


impl BuildConfig {
    /// creates a new instance of the tailwind config with default values
    pub fn new() -> Self {
        Self {
            css_path: None, // style.css
            tailwind_config: serde_json::json!({
                "content": ["{src_dir}/**/*.{html,js,rs}"],
                "theme": { "extend": {} },
                "plugins": [],
            }),
            cdn_src: format!("https://cdn.tailwindcss.com"),
            always: false,
        }
    }

    /// changes the path from which the css file is loaded
    /// specifying a file makes it required
    /// specifying `None` looks for a `style.css` file
    /// at the root of of the project
    /// if the file does not exist it uses the the default:
    /// ```css
    /// @tailwind base;
    /// @tailwind components;
    /// @tailwind utilities;
    /// ```
    pub fn with_path(mut self, p: Option<impl AsRef<Path>>) -> Self {
        self.css_path = p.map(|v| v.as_ref().to_path_buf()); self
    }

    /// specifies the cdn used as a source for the jit builds
    pub fn with_cdn_src(mut self, s: impl Into<String>) -> Self {
        self.cdn_src = s.into(); self
    }

    /// specifies the config used by tailwind, the config needs to be specified as json
    /// as it is used by both the jit and the normal config
    /// (`{src_dir}` expands to the actual `/src` of the project)
    ///
    /// ```rust
    /// // default config:
    /// json!({
    ///     "content": ["{src_dir}/**/*.{html,js,rs}"],
    ///     "theme": { "extend": {} },
    ///     "plugins": [],
    /// })
    /// ```
    pub fn with_tw_config(mut self, config: serde_json::Value) -> Self {
        self.tailwind_config = config; self
    }

    /// always rebuilds tailwind, never uses jit
    /// (corosponds to the `include_tailwind!(always)` macro)
    pub fn always(mut self) -> Self { self.always = true; self }

    fn is_release() -> bool {
        println!("cargo:rerun-if-env-changed=PROFILE");

        match env::var("PROFILE").as_ref().map(|v| v.as_str()) {
            Ok("release") => true,
            Ok("debug") => false,
            Ok(v) => {
                println!("cargo:warning='PROFILE' was neither release nor debug ('{v}')");
                false
            },
            Err(_) => {
                println!("cargo:warning='PROFILE' was not defined, defaulting to debug");
                false
            },
        }
    }

    const DEFAULT_PACKAGE_JSON: &'static str = r#"{
    "name": "include-tailwind",
    "version": "1.0.0",
    "description": "the autogenerated package.json for include-tailwind",
    "devDependencies": {
        "tailwindcss": "^3.4.4"
    }
}
"#;

    const DEFAULT_STYLE_CSS: &'static str = r#"
@tailwind base;
@tailwind components;
@tailwind utilities;
"#;

    fn config_string(&self, src_dir: &Path) -> Result<String, Error> {
        let config_string = serde_json::to_string_pretty(&self.tailwind_config)
            .expect("could not serialize tailwind config")
            .replace("{src_dir}", src_dir.to_str().ok_or(Error::InvalidSrcPath)?);

        Ok(config_string)
    }

    fn install_tailwind(&self, out_dir: &Path, src_dir: &Path) -> Result<(), Error> {
        let package_json_path = out_dir.join("package.json");
        let node_modules_path = out_dir.join("node_modules");
        let tw_config_path = out_dir.join("tailwind.config.js");

        if !package_json_path.exists() {
            println!("creating package.json ({package_json_path:?})");
            std::fs::write(&package_json_path, Self::DEFAULT_PACKAGE_JSON)?;
        } else { println!("package.json already exists, not creating another one") }

        if !node_modules_path.exists() {
            println!("installing tailwind");
            if !Command::new("npm").args(["install"])
                .current_dir(out_dir)
                .status()
            .unwrap().success() { panic!("could not install tailwind") }
        } else { println!("node_modules already exists, not installing") }

        println!("writing tailwind config ({tw_config_path:?})");
        let config_string = self.config_string(src_dir)?;
        let config = format!("
            module.exports = {config_string}
        ");
        std::fs::write(&tw_config_path, config)?;

        Ok(())
    }

    fn compile_tailwind(&self, out_dir: &Path) -> Result<(), Error> {
        let tw_in_path = out_dir.join("style.in.css");
        let tw_out_path = out_dir.join("style.css");

        if let Some(p) = &self.css_path {
            println!("cargo:rerun-if-changed={}", p.to_string_lossy());
            if p.exists() {
                println!("copying {p:?} to build css");
                std::fs::copy(p, &tw_in_path)?;
            } else { panic!("specified a css path but it does not exists") }
        } else {
            let default_style_path = PathBuf::from("style.css");
            if default_style_path.exists() {
                println!("copying style.css (default path)");
                std::fs::copy(&default_style_path, &tw_in_path)?;
            } else {
                println!("creating default style.css");
                std::fs::write(&tw_in_path, Self::DEFAULT_STYLE_CSS)?;
            }
        }

        if !Command::new("npx")
            .args(["tailwindcss"])
            .arg("-i").arg(&tw_in_path)
            .arg("-o").arg(&tw_out_path)
            .args(["--minify"])
            .current_dir(out_dir)
            .status().unwrap()
        .success() {
            panic!("could not build styles");
        }

        println!("cargo:rustc-env=INCLUDE_TAILWIND_PATH={}", tw_out_path.to_str().unwrap());

        Ok(())
    }

    // https://tailwindcss.com/docs/installation/play-cdn
    fn setup_jit(&self, out_dir: &Path, src_dir: &Path) -> Result<(), Error> {
        let config_string = self.config_string(src_dir)?;
        let jit_config_path = out_dir.join("jit_config.js");
        let config = format!("tailwind.config = {config_string}");
        std::fs::write(&jit_config_path, config)?;
        println!("cargo:rustc-env=INCLUDE_TAILWIND_JIT_CONFIG_PATH={}",
            jit_config_path.to_str().unwrap());

        println!("cargo:rustc-env=INCLUDE_TAILWIND_JIT_URL={}", self.cdn_src);

        Ok(())
    }

    /// builds tailwind using the specified config
    pub fn build(&self) -> Result<(), Error> {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not provided"));
        let src_dir = std::fs::canonicalize("./src").expect("could not canonicalize");
        let release = Self::is_release();


        if release || self.always {
            self.install_tailwind(&out_dir, &src_dir)?;
            self.compile_tailwind(&out_dir)?;
        } else {
            self.setup_jit(&out_dir, &src_dir)?;
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("the source dir contained invalid unicode")]
    InvalidSrcPath,
    #[error("tailwind could not be installed")]
    TailwindInstallError,
}

/// builds tailwind with the default config
pub fn build_tailwind() -> Result<(), Error> { BuildConfig::default().build() }

