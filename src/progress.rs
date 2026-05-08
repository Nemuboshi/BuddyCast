use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

/// Lightweight progress handle that can be disabled for nested or transient operations.
#[derive(Debug, Clone)]
pub struct ProgressHandle {
    bar: ProgressBar,
    enabled: bool,
}

impl ProgressHandle {
    /// Advance the progress indicator by a byte count when visible.
    pub fn inc(&self, amount: u64) {
        if self.enabled {
            self.bar.inc(amount);
        }
    }

    /// Finish and clear the progress display when visible.
    pub fn finish_and_clear(&self) {
        if self.enabled {
            self.bar.finish_and_clear();
        }
    }

    /// Abandon the progress display with a final error message when visible.
    pub fn abandon_with_message(&self, message: &str) {
        if self.enabled {
            self.bar.abandon_with_message(message.to_string());
        }
    }
}

/// Create a spinner used for steps where the total size is unknown.
pub fn spinner(message: &str) -> ProgressHandle {
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.set_message(message.to_string());
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    ProgressHandle {
        bar: progress_bar,
        enabled: true,
    }
}

/// Create a byte-based progress bar for download and processing steps.
pub fn bytes_bar(total_bytes: u64, message: &str) -> ProgressHandle {
    let progress_bar = ProgressBar::new(total_bytes);
    progress_bar.set_message(message.to_string());
    progress_bar.set_style(
        ProgressStyle::with_template(
            "{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA {eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-"),
    );
    ProgressHandle {
        bar: progress_bar,
        enabled: true,
    }
}

/// Create a count-based progress bar for file-oriented batch work.
pub fn count_bar(total_items: u64, message: &str) -> ProgressHandle {
    let progress_bar = ProgressBar::new(total_items);
    progress_bar.set_message(message.to_string());
    progress_bar.set_style(
        ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-"),
    );
    ProgressHandle {
        bar: progress_bar,
        enabled: true,
    }
}

/// Create a hidden progress handle for nested work that should not redraw the terminal.
pub fn hidden() -> ProgressHandle {
    ProgressHandle {
        bar: ProgressBar::hidden(),
        enabled: false,
    }
}
