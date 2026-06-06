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
    cdn_src: String,
    tailwind_version: String,
}


impl BuildConfig {
    /// creates a new instance of the tailwind config with default values
    pub fn new() -> Self {
        Self {
            css_path: None, // style.css
            cdn_src: format!("https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"),
            tailwind_version: format!("@tailwindcss/cli@4.3.0"),
            always: false,
        }
    }

    /// specify the tailwind cli package to use
    ///
    /// default: `@tailwindcss/cli@4.3.0`
    pub fn with_tailwind_package(mut self, version: impl Into<String>) -> Self {
        self.tailwind_version = version.into(); self
    }

    /// changes the path from which the css file is loaded
    /// specifying a file makes it required
    /// specifying `None` looks for a `style.css` file
    /// at the root of of the project
    /// if the file does not exist it uses the the default:
    /// ```css
    /// @import "tailwindcss";
    /// @source "src/**/*.{rs,html,js}";
    /// ```
    pub fn with_path(mut self, p: Option<impl AsRef<Path>>) -> Self {
        self.css_path = p.map(|v| v.as_ref().to_path_buf()); self
    }

    /// specifies the cdn used as a source for the jit builds
    pub fn with_cdn_src(mut self, s: impl Into<String>) -> Self {
        self.cdn_src = s.into(); self
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

    const DEFAULT_STYLE_CSS: &'static str = r#"
@import "tailwindcss";
@source "{src_dir}/**/*.{rs,html,js}"
"#;

    fn css_path(&self) -> PathBuf {
        let p = if let Some(css_path) = &self.css_path {
            css_path.clone()
        } else {
            let default_p = PathBuf::from("style.css");
            if default_p.exists() { default_p }
            else {
                let temp_p = PathBuf::from(std::env::var("OUT_DIR").unwrap())
                    .join("style.css");

                if !temp_p.exists() {
                    std::fs::write(&temp_p, Self::DEFAULT_STYLE_CSS)
                        .expect("could not write temp style.css file");
                }

                temp_p
            }
        };
        println!("cargo:rerun-if-changed={}", p.to_str().unwrap());
        p
    }

    fn write_css_string(&self, out_dir: &Path, src_dir: &Path) -> Result<(), Error> {
        let css_path = self.css_path();
        let css_string = std::fs::read_to_string(&css_path)
            .map_err(|err| Error::StyleCssNotFound(css_path, err))?
            .replace("{src_dir}", src_dir.to_str().ok_or(Error::InvalidSrcPath)?);
        std::fs::write(out_dir.join("style.in.css"), css_string)?;
        Ok(())
    }

    fn compile_tailwind(&self, out_dir: &Path) -> Result<(), Error> {
        let tw_in_path = out_dir.join("style.in.css");
        let tw_out_path = out_dir.join("style.css");

        if !Command::new("npx")
            .args([&self.tailwind_version])
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
    fn setup_jit(&self, out_dir: &Path) -> Result<(), Error> {
        let jit_config_path = out_dir.join("style.in.css");

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

        self.write_css_string(&out_dir, &src_dir)?;

        if release || self.always {
            self.compile_tailwind(&out_dir)?;
        } else {
            self.setup_jit(&out_dir)?;
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
    #[error("could not read style.css at '{0}' -> {1}")]
    StyleCssNotFound(PathBuf, std::io::Error),
    #[error("tailwind could not be installed")]
    TailwindInstallError,
}

/// builds tailwind with the default config
pub fn build_tailwind() -> Result<(), Error> { BuildConfig::default().build() }

