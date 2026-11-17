//! Frame-rate bookkeeping for the native Vantage HUD. Pure: callers pass `now`.

use std::collections::VecDeque;

/// Seconds of frames one FPS reading averages over.
const FPS_WINDOW: f64 = 0.5;

/// Frames per second averaged over the last half second.
#[derive(Default)]
pub(super) struct Fps {
    frames: VecDeque<f64>,
}

impl Fps {
    /// Records a frame that ended at `now` and returns the rounded rate.
    pub(super) fn frame(&mut self, now: f64) -> u32 {
        self.frames.push_back(now);
        while self
            .frames
            .front()
            .is_some_and(|start| now - start > FPS_WINDOW)
        {
            self.frames.pop_front();
        }
        let count = self.frames.len();
        let Some(first) = self.frames.front() else {
            return 0;
        };
        let span = now - first;
        if count < 2 || span <= 0.0 {
            return 0;
        }
        (((count - 1) as f64) / span).round() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_sixty_frames_a_second_reads_sixty() {
        let mut fps = Fps::default();
        let mut last = 0;
        for frame in 0..120 {
            last = fps.frame(frame as f64 / 60.0);
        }
        assert_eq!(last, 60);
    }

    #[test]
    fn a_single_frame_reads_zero() {
        assert_eq!(Fps::default().frame(1.0), 0);
    }
}
