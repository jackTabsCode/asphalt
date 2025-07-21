use indicatif::{MultiProgress, ProgressBar as InnerProgressBar, ProgressStyle};

#[derive(Debug, Clone)]
pub struct ProgressBar {
    inner: InnerProgressBar,
}

impl ProgressBar {
    pub fn new(mp: MultiProgress, prefix: &str, len: usize) -> Self {
        let template = "{prefix:>.bold}\n[{bar:40.cyan/blue}] {pos}/{len}: {msg} ({eta})";

        let inner = mp.add(InnerProgressBar::new(len as u64));

        inner.set_style(
            ProgressStyle::default_bar()
                .template(template)
                .unwrap()
                .progress_chars("=>"),
        );
        inner.set_prefix(prefix.to_string());

        inner.tick();

        Self { inner }
    }

    pub fn set_msg(&self, msg: impl Into<String>) {
        self.inner.set_message(msg.into());
    }

    pub fn inc(&self, delta: u64) {
        self.inner.inc(delta);
    }

    pub fn finish(&self) {
        self.inner.finish_and_clear();
    }
}
