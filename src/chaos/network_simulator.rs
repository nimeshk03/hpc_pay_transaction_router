use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkCondition {
    Normal,
    HighLatency,
    PacketLoss,
    Jitter,
    Partition,
    Throttled,
}

impl NetworkCondition {
    pub fn description(&self) -> &'static str {
        match self {
            NetworkCondition::Normal => "Normal network conditions",
            NetworkCondition::HighLatency => "High latency network",
            NetworkCondition::PacketLoss => "Packet loss occurring",
            NetworkCondition::Jitter => "Network jitter present",
            NetworkCondition::Partition => "Network partition active",
            NetworkCondition::Throttled => "Bandwidth throttled",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LatencyProfile {
    pub base_latency_ms: u64,
    pub jitter_ms: u64,
    pub packet_loss_rate: f64,
    pub bandwidth_limit_kbps: Option<u64>,
}

impl LatencyProfile {
    pub fn normal() -> Self {
        Self {
            base_latency_ms: 10,
            jitter_ms: 5,
            packet_loss_rate: 0.0,
            bandwidth_limit_kbps: None,
        }
    }

    pub fn high_latency() -> Self {
        Self {
            base_latency_ms: 500,
            jitter_ms: 100,
            packet_loss_rate: 0.01,
            bandwidth_limit_kbps: None,
        }
    }

    pub fn degraded() -> Self {
        Self {
            base_latency_ms: 200,
            jitter_ms: 50,
            packet_loss_rate: 0.05,
            bandwidth_limit_kbps: Some(1000),
        }
    }

    pub fn severe() -> Self {
        Self {
            base_latency_ms: 1000,
            jitter_ms: 500,
            packet_loss_rate: 0.2,
            bandwidth_limit_kbps: Some(100),
        }
    }

    pub fn calculate_latency(&self) -> Duration {
        let jitter: i64 = if self.jitter_ms > 0 {
            let random: f64 = rand::random();
            ((random * 2.0 - 1.0) * self.jitter_ms as f64) as i64
        } else {
            0
        };

        let total_ms = (self.base_latency_ms as i64 + jitter).max(0) as u64;
        Duration::from_millis(total_ms)
    }

    pub fn should_drop_packet(&self) -> bool {
        if self.packet_loss_rate <= 0.0 {
            return false;
        }
        let random: f64 = rand::random();
        random < self.packet_loss_rate
    }
}

impl Default for LatencyProfile {
    fn default() -> Self {
        Self::normal()
    }
}

#[derive(Debug, Clone)]
struct NetworkState {
    condition: NetworkCondition,
    profile: LatencyProfile,
    start_time: std::time::Instant,
    duration: Option<Duration>,
}

pub struct NetworkSimulator {
    states: Arc<Mutex<HashMap<String, NetworkState>>>,
    global_condition: Arc<Mutex<Option<NetworkState>>>,
}

impl NetworkSimulator {
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
            global_condition: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_condition(&self, target: &str, condition: NetworkCondition, profile: LatencyProfile) {
        let mut states = self.states.lock().unwrap();
        states.insert(
            target.to_string(),
            NetworkState {
                condition,
                profile,
                start_time: std::time::Instant::now(),
                duration: None,
            },
        );
    }

    pub fn set_condition_with_duration(
        &self,
        target: &str,
        condition: NetworkCondition,
        profile: LatencyProfile,
        duration: Duration,
    ) {
        let mut states = self.states.lock().unwrap();
        states.insert(
            target.to_string(),
            NetworkState {
                condition,
                profile,
                start_time: std::time::Instant::now(),
                duration: Some(duration),
            },
        );
    }

    pub fn set_global_condition(&self, condition: NetworkCondition, profile: LatencyProfile) {
        let mut global = self.global_condition.lock().unwrap();
        *global = Some(NetworkState {
            condition,
            profile,
            start_time: std::time::Instant::now(),
            duration: None,
        });
    }

    pub fn clear_condition(&self, target: &str) {
        let mut states = self.states.lock().unwrap();
        states.remove(target);
    }

    pub fn clear_global_condition(&self) {
        let mut global = self.global_condition.lock().unwrap();
        *global = None;
    }

    pub fn clear_all(&self) {
        let mut states = self.states.lock().unwrap();
        states.clear();
        let mut global = self.global_condition.lock().unwrap();
        *global = None;
    }

    pub fn get_condition(&self, target: &str) -> NetworkCondition {
        let states = self.states.lock().unwrap();

        if let Some(state) = states.get(target) {
            if let Some(duration) = state.duration {
                if state.start_time.elapsed() >= duration {
                    return NetworkCondition::Normal;
                }
            }
            return state.condition;
        }

        let global = self.global_condition.lock().unwrap();
        if let Some(state) = global.as_ref() {
            if let Some(duration) = state.duration {
                if state.start_time.elapsed() >= duration {
                    return NetworkCondition::Normal;
                }
            }
            return state.condition;
        }

        NetworkCondition::Normal
    }

    pub fn get_latency(&self, target: &str) -> Duration {
        let states = self.states.lock().unwrap();

        if let Some(state) = states.get(target) {
            if let Some(duration) = state.duration {
                if state.start_time.elapsed() >= duration {
                    return LatencyProfile::normal().calculate_latency();
                }
            }
            return state.profile.calculate_latency();
        }

        drop(states);

        let global = self.global_condition.lock().unwrap();
        if let Some(state) = global.as_ref() {
            if let Some(duration) = state.duration {
                if state.start_time.elapsed() >= duration {
                    return LatencyProfile::normal().calculate_latency();
                }
            }
            return state.profile.calculate_latency();
        }

        LatencyProfile::normal().calculate_latency()
    }

    pub fn should_drop(&self, target: &str) -> bool {
        let states = self.states.lock().unwrap();

        if let Some(state) = states.get(target) {
            if let Some(duration) = state.duration {
                if state.start_time.elapsed() >= duration {
                    return false;
                }
            }
            return state.profile.should_drop_packet();
        }

        drop(states);

        let global = self.global_condition.lock().unwrap();
        if let Some(state) = global.as_ref() {
            if let Some(duration) = state.duration {
                if state.start_time.elapsed() >= duration {
                    return false;
                }
            }
            return state.profile.should_drop_packet();
        }

        false
    }

    pub fn simulate_partition(&self, targets: &[&str]) {
        for target in targets {
            self.set_condition(
                target,
                NetworkCondition::Partition,
                LatencyProfile {
                    base_latency_ms: 0,
                    jitter_ms: 0,
                    packet_loss_rate: 1.0,
                    bandwidth_limit_kbps: None,
                },
            );
        }
    }

    pub fn heal_partition(&self, targets: &[&str]) {
        for target in targets {
            self.clear_condition(target);
        }
    }

    pub fn get_all_conditions(&self) -> HashMap<String, NetworkCondition> {
        let states = self.states.lock().unwrap();
        states
            .iter()
            .map(|(k, v)| (k.clone(), v.condition))
            .collect()
    }
}

impl Default for NetworkSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for NetworkSimulator {
    fn clone(&self) -> Self {
        Self {
            states: self.states.clone(),
            global_condition: self.global_condition.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_profile_normal() {
        let profile = LatencyProfile::normal();
        assert_eq!(profile.base_latency_ms, 10);
        assert_eq!(profile.packet_loss_rate, 0.0);
    }

    #[test]
    fn test_latency_profile_calculate() {
        let profile = LatencyProfile::normal();
        let latency = profile.calculate_latency();
        assert!(latency.as_millis() <= 20);
    }

    #[test]
    fn test_network_simulator_set_condition() {
        let simulator = NetworkSimulator::new();

        simulator.set_condition("stripe", NetworkCondition::HighLatency, LatencyProfile::high_latency());

        assert_eq!(simulator.get_condition("stripe"), NetworkCondition::HighLatency);
        assert_eq!(simulator.get_condition("adyen"), NetworkCondition::Normal);
    }

    #[test]
    fn test_network_simulator_global_condition() {
        let simulator = NetworkSimulator::new();

        simulator.set_global_condition(NetworkCondition::Throttled, LatencyProfile::degraded());

        assert_eq!(simulator.get_condition("stripe"), NetworkCondition::Throttled);
        assert_eq!(simulator.get_condition("adyen"), NetworkCondition::Throttled);
    }

    #[test]
    fn test_network_simulator_target_overrides_global() {
        let simulator = NetworkSimulator::new();

        simulator.set_global_condition(NetworkCondition::Throttled, LatencyProfile::degraded());
        simulator.set_condition("stripe", NetworkCondition::Partition, LatencyProfile::severe());

        assert_eq!(simulator.get_condition("stripe"), NetworkCondition::Partition);
        assert_eq!(simulator.get_condition("adyen"), NetworkCondition::Throttled);
    }

    #[test]
    fn test_network_simulator_clear() {
        let simulator = NetworkSimulator::new();

        simulator.set_condition("stripe", NetworkCondition::HighLatency, LatencyProfile::high_latency());
        assert_eq!(simulator.get_condition("stripe"), NetworkCondition::HighLatency);

        simulator.clear_condition("stripe");
        assert_eq!(simulator.get_condition("stripe"), NetworkCondition::Normal);
    }

    #[test]
    fn test_network_simulator_partition() {
        let simulator = NetworkSimulator::new();

        simulator.simulate_partition(&["stripe", "adyen"]);

        assert_eq!(simulator.get_condition("stripe"), NetworkCondition::Partition);
        assert_eq!(simulator.get_condition("adyen"), NetworkCondition::Partition);
        assert!(simulator.should_drop("stripe"));
    }

    #[test]
    fn test_network_simulator_heal_partition() {
        let simulator = NetworkSimulator::new();

        simulator.simulate_partition(&["stripe"]);
        assert_eq!(simulator.get_condition("stripe"), NetworkCondition::Partition);

        simulator.heal_partition(&["stripe"]);
        assert_eq!(simulator.get_condition("stripe"), NetworkCondition::Normal);
    }

    #[test]
    fn test_network_condition_descriptions() {
        assert!(!NetworkCondition::Normal.description().is_empty());
        assert!(!NetworkCondition::Partition.description().is_empty());
    }
}
