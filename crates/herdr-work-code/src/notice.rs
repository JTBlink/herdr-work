//! Footer messages expire independently of keyboard or mouse input.
use std::time::{Duration, Instant};

pub const NOTICE_DURATION: Duration = Duration::from_secs(3);

pub struct Notice<T> {
    value: Option<(T, Instant)>,
}

impl<T> Default for Notice<T> {
    fn default() -> Self {
        Self { value: None }
    }
}

impl<T> Notice<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Some((value, Instant::now())),
        }
    }

    pub fn as_ref(&self) -> Option<&T> {
        self.at(Instant::now())
    }

    pub fn is_none(&self) -> bool {
        self.as_ref().is_none()
    }

    fn at(&self, now: Instant) -> Option<&T> {
        self.value
            .as_ref()
            .filter(|(_, shown)| now.saturating_duration_since(*shown) < NOTICE_DURATION)
            .map(|(value, _)| value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_notice_expires_at_the_deadline_without_input() {
        let notice = Notice::new("opened");
        let shown = notice.value.as_ref().unwrap().1;
        assert_eq!(
            notice.at(shown + NOTICE_DURATION - Duration::from_millis(1)),
            Some(&"opened")
        );
        assert_eq!(notice.at(shown + NOTICE_DURATION), None);
        assert_eq!(
            notice.at(shown + NOTICE_DURATION + Duration::from_secs(20)),
            None
        );
    }

    #[test]
    fn repeated_message_gets_a_fresh_deadline_and_clearing_is_immediate() {
        let first = Instant::now();
        let mut notice = Notice {
            value: Some(("opened", first)),
        };
        let repeated = first + Duration::from_secs(2);
        notice.value = Some(("opened", repeated));
        assert_eq!(notice.at(first + NOTICE_DURATION), Some(&"opened"));
        assert_eq!(notice.at(repeated + NOTICE_DURATION), None);
        notice = Notice::default();
        assert_eq!(notice.at(repeated), None);
    }
}
