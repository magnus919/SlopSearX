//! Browser child supervisor. Cancellation never discards the actual Child handle.
use crate::Result;
#[derive(Default)]
pub struct BrowserOwner {
    child: tokio::sync::Mutex<Option<tokio::process::Child>>,
}
impl BrowserOwner {
    pub async fn open(&self, url: &str) -> Result<()> {
        let mut child = self.child.lock().await;
        if let Some(child) = child.as_mut()
            && child.try_wait()?.is_none()
        {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        let mut command = tokio::process::Command::new("open");
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut c = tokio::process::Command::new("rundll32");
            c.arg("url.dll,FileProtocolHandler");
            c
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let mut command = tokio::process::Command::new("xdg-open");
        *child = Some(
            command
                .arg(url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn()?,
        );
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<()> {
        let mut slot = self.child.lock().await;
        if let Some(child) = slot.as_mut() {
            if child.try_wait()?.is_none() {
                child.start_kill()?;
            }
            child.wait().await?;
            slot.take();
        }
        Ok(())
    }
}
