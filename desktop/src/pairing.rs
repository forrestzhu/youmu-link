//! Logical pairing windows survive a failed TCP attempt, never its deadline.
use rand::Rng;
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};
use ymlink_core::session::{new_pairing_code, AuthErrorCode, PAIRING_TTL};
use zeroize::Zeroizing;

#[derive(Clone)]
pub(crate) struct DisplayWindow {
    pub id: [u8; 16],
    pub peer_key: [u8; 32],
    pub code: Zeroizing<String>,
    pub deadline: Instant,
}

#[derive(Clone)]
pub(crate) struct EnterWindow {
    pub address: SocketAddr,
    pub id: [u8; 16],
    pub peer_key: [u8; 32],
    pub deadline: Instant,
}

#[derive(Clone)]
pub(crate) struct EnterTarget {
    pub address: SocketAddr,
    pub peer_key: [u8; 32],
}

#[derive(Default)]
pub(crate) struct Windows {
    display: Option<DisplayWindow>,
    pub enter: Option<EnterWindow>,
    pub target: Option<EnterTarget>,
}

impl Windows {
    pub fn contains_key(&self, key: &[u8; 32]) -> bool {
        self.display.as_ref().is_some_and(|w| &w.peer_key == key)
            || self.enter.as_ref().is_some_and(|w| &w.peer_key == key)
            || self.target.as_ref().is_some_and(|w| &w.peer_key == key)
    }
    pub fn display(
        &mut self,
        peer_key: [u8; 32],
        resume: Option<[u8; 16]>,
        now: Instant,
    ) -> Result<DisplayWindow, AuthErrorCode> {
        if self.display.as_ref().is_some_and(|w| now >= w.deadline) {
            self.display.take();
        }
        if let Some(id) = resume {
            return self
                .display
                .as_ref()
                .filter(|w| w.id == id && w.peer_key == peer_key)
                .cloned()
                .ok_or(AuthErrorCode::Expired);
        }
        if self
            .display
            .as_ref()
            .is_some_and(|w| w.peer_key != peer_key)
        {
            return Err(AuthErrorCode::Busy);
        }
        let window = DisplayWindow {
            id: rand::rngs::OsRng.gen(),
            peer_key,
            code: Zeroizing::new(new_pairing_code()),
            deadline: now + PAIRING_TTL,
        };
        self.display = Some(window.clone());
        Ok(window)
    }

    pub fn clear(&mut self) {
        self.display.take();
        self.enter.take();
        self.target.take();
    }

    pub fn clear_peer(&mut self, id: &str) {
        if self
            .target
            .as_ref()
            .is_some_and(|w| ymlink_core::identity::device_id(&w.peer_key) == id)
        {
            self.target.take();
        }
        if self
            .display
            .as_ref()
            .is_some_and(|w| ymlink_core::identity::device_id(&w.peer_key) == id)
        {
            self.display.take();
        }
        if self
            .enter
            .as_ref()
            .is_some_and(|w| ymlink_core::identity::device_id(&w.peer_key) == id)
        {
            self.enter.take();
        }
    }

    pub fn invalidate_peer(&mut self, id: &str) {
        if self
            .display
            .as_ref()
            .is_some_and(|w| ymlink_core::identity::device_id(&w.peer_key) == id)
        {
            self.display.take();
        }
        // Retain only target, never the old id, after a terminal result.
        if self
            .enter
            .as_ref()
            .is_some_and(|w| ymlink_core::identity::device_id(&w.peer_key) == id)
        {
            self.enter.take();
        }
    }
}

#[derive(Default)]
pub(crate) struct Budget {
    entries: HashMap<IpAddr, VecDeque<Instant>>,
}

impl Budget {
    pub fn admit(&mut self, ip: IpAddr, now: Instant) -> Result<(), u32> {
        let period = Duration::from_secs(60);
        self.entries.retain(|_, entries| {
            entries
                .back()
                .is_some_and(|at| now.saturating_duration_since(*at) < period)
        });
        if self.entries.len() >= 128 && !self.entries.contains_key(&ip) {
            return Err(60_000);
        }
        let entries = self.entries.entry(ip).or_default();
        while entries
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= period)
        {
            entries.pop_front();
        }
        if entries.len() >= 5 {
            let first = entries.front().ok_or(60_000u32)?;
            let remaining = period.saturating_sub(now.saturating_duration_since(*first));
            return Err((remaining.as_millis()
                + u128::from(remaining.subsec_nanos() % 1_000_000 != 0))
            .clamp(1, 60_000) as u32);
        }
        entries.push_back(now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_reuses_only_same_peer_window_and_original_deadline() {
        let now = Instant::now();
        let mut windows = Windows::default();
        let first = windows.display([1; 32], None, now).unwrap();
        let second = windows
            .display([1; 32], Some(first.id), now + Duration::from_secs(20))
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(*first.code, *second.code);
        assert_eq!(first.deadline, second.deadline);
        assert!(matches!(
            windows.display([2; 32], Some(first.id), now),
            Err(AuthErrorCode::Expired)
        ));
        assert!(matches!(
            windows.display([1; 32], Some([0; 16]), now),
            Err(AuthErrorCode::Expired)
        ));
        assert!(matches!(
            windows.display([2; 32], None, now),
            Err(AuthErrorCode::Busy)
        ));
    }

    #[test]
    fn expiry_or_cancel_never_silently_recreates_resumed_window() {
        let now = Instant::now();
        let mut windows = Windows::default();
        let first = windows.display([1; 32], None, now).unwrap();
        assert!(matches!(
            windows.display([1; 32], Some(first.id), first.deadline),
            Err(AuthErrorCode::Expired)
        ));
        let fresh = windows.display([1; 32], None, first.deadline).unwrap();
        assert_ne!(fresh.id, first.id);
        windows.clear();
        assert!(matches!(
            windows.display([1; 32], Some(fresh.id), first.deadline),
            Err(AuthErrorCode::Expired)
        ));
    }

    #[test]
    fn forgetting_one_peer_does_not_clear_another_pairing_window() {
        let now = Instant::now();
        let mut windows = Windows::default();
        let first = windows.display([1; 32], None, now).unwrap();
        windows.clear_peer(&ymlink_core::identity::device_id(&[2; 32]));
        assert!(windows.display([1; 32], Some(first.id), now).is_ok());
        windows.clear_peer(&ymlink_core::identity::device_id(&[1; 32]));
        assert!(matches!(
            windows.display([1; 32], Some(first.id), now),
            Err(AuthErrorCode::Expired)
        ));
    }

    #[test]
    fn budget_follows_ip_not_connection_and_recovers_at_exact_boundary() {
        let now = Instant::now();
        let ip = "192.0.2.1".parse().unwrap();
        let mut budget = Budget::default();
        for _ in 0..5 {
            budget.admit(ip, now).unwrap();
        }
        assert_eq!(budget.admit(ip, now), Err(60_000));
        assert_eq!(
            budget.admit(ip, now + Duration::from_millis(59_999)),
            Err(1)
        );
        budget.admit(ip, now + Duration::from_secs(60)).unwrap();
        budget.admit("192.0.2.2".parse().unwrap(), now).unwrap();
    }

    #[test]
    fn new_windows_or_cancel_do_not_reset_attempt_budget() {
        let now = Instant::now();
        let ip = "192.0.2.1".parse().unwrap();
        let mut budget = Budget::default();
        let mut windows = Windows::default();
        for _ in 0..5 {
            budget.admit(ip, now).unwrap();
            windows.display([1; 32], None, now).unwrap();
            windows.clear();
        }
        assert_eq!(budget.admit(ip, now), Err(60_000));
    }
}
