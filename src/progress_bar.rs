use indicatif::{MultiProgress, ProgressBar as InnerProgressBar, ProgressStyle};

#[derive(Debug, Clone)]
pub struct ProgressBar {
    inner: InnerProgressBar,
}

// mpiling image v0.25.6
//     Building [=======================> ] 374/376: image

impl ProgressBar {
    pub fn new(mp: MultiProgress, prefix: &str, len: usize) -> Self {
        let template = "{prefix:>.bold}\n[{bar:40.cyan/blue}] {pos}/{len}: {msg} ({eta})";

        let inner = InnerProgressBar::new(len as u64)
            .with_prefix(prefix.to_string())
            .with_style(
                ProgressStyle::default_bar()
                    .template(template)
                    .unwrap()
                    .progress_chars("=>"),
            );

        let inner = mp.add(inner);
        inner.tick();

        Self { inner }
    }

    pub fn set_msg(&self, msg: impl Into<String>) {
        self.inner.set_message(msg.into());
    }

    pub fn inc(&self, delta: u64) {
        self.inner.inc(delta);
    }

    pub fn finish_with_message(&self, msg: impl Into<String>) {
        self.inner.finish_with_message(msg.into());
    }
}
