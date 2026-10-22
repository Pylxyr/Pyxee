//! Sleep/resume and network changes, found by polling (no Windows messages needed). Decisions: `pyxee_shell::power`.

use super::ShellState;
use crate::dto::TransportDto;
use crate::state::{now_ms, publish};
use pyxee_core::Command;
use pyxee_shell::power::{
    decide, Action, NetState, NetworkWatcher, ResumeDetector, Situation, Trigger,
};
use std::net::UdpSocket;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const POLL: Duration = Duration::from_secs(2);
/// A gap this much longer than the poll interval means the machine was asleep (or frozen).
const GAP_MS: u64 = 8_000;

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut resume = ResumeDetector::new(GAP_MS);
        let mut net = NetworkWatcher::new();
        let mut ticks: u64 = 0;
        loop {
            std::thread::sleep(POLL);
            ticks += 1;
            // The second clock is "how much time should have passed": a wall clock that ran ahead of it
            // means this thread did not run, which is exactly what sleep and hibernate do.
            let wall = now_ms().max(0) as u64;
            if let Some(slept) = resume.tick(wall, ticks * POLL.as_millis() as u64) {
                log::info!("the PC woke up after about {} s", slept / 1000);
                handle(&app, Trigger::Resumed);
            }
            if let Some(ev) = net.observe(probe()) {
                log::info!("network: {ev:?}");
                handle(&app, Trigger::Net(ev));
            }
        }
    });
}

/// Is there a route out, and through which local address? (No packet is sent: UDP `connect` only picks a route.)
fn probe() -> NetState {
    let local = UdpSocket::bind("0.0.0.0:0").and_then(|s| {
        s.connect("1.1.1.1:53")?;
        s.local_addr()
    });
    match local {
        Ok(addr) => NetState {
            online: true,
            local_addr: Some(addr.ip().to_string()),
        },
        Err(_) => NetState {
            online: false,
            local_addr: None,
        },
    }
}

fn situation(app: &AppHandle) -> Situation {
    let Some(state) = app.try_state::<crate::state::AppState>() else {
        return Situation::Other;
    };
    let Ok(p) = state.player.lock() else {
        return Situation::Other;
    };
    match crate::dto::state_dto(&p).transport {
        TransportDto::Playing { .. } => Situation::Playing,
        TransportDto::Failed { failure, .. } => {
            use crate::dto::FailureDto::*;
            if matches!(failure, Offline | RateLimited | Expired { .. }) {
                Situation::FailedRetryable
            } else {
                Situation::Other
            }
        }
        _ => Situation::Other,
    }
}

fn handle(app: &AppHandle, trigger: Trigger) {
    if app.try_state::<ShellState>().is_none() {
        return;
    }
    for action in decide(trigger, situation(app)) {
        match action {
            Action::Retry => publish(app, |p| p.command(Command::Retry)),
            Action::Notice(message) => {
                let _ = super::emit_noted(
                    app,
                    crate::state::NOTICE_EVENT,
                    crate::dto::NoticeDto {
                        message: message.to_string(),
                    },
                );
            }
        }
    }
}
