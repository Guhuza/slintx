use slint::PlatformError;

slint::include_modules!();

pub fn run() -> Result<(), PlatformError> {
    let window = AppWindow::new()?;
    window.run()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: slint::android::AndroidApp) {
    if let Err(error) = slint::android::init(app) {
        eprintln!("failed to initialize Android backend: {error}");
        return;
    }
    if let Err(error) = run() {
        eprintln!("application error: {error}");
    }
}
