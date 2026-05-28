// Multi-monitor enumeration via xcap.

use crate::commands::MonitorInfo;
use anyhow::Result;
use xcap::Monitor;

pub fn list_monitors() -> Result<Vec<MonitorInfo>> {
    let monitors = Monitor::all()?;
    let mut out = Vec::with_capacity(monitors.len());
    for (index, m) in monitors.iter().enumerate() {
        out.push(MonitorInfo {
            index,
            name: m.name().unwrap_or_default().to_string(),
            width: m.width().unwrap_or(0),
            height: m.height().unwrap_or(0),
            is_primary: m.is_primary().unwrap_or(false),
        });
    }
    Ok(out)
}

/// Get a monitor by zero-based index, falling back to primary.
pub fn get_monitor(index: usize) -> Result<Monitor> {
    let monitors = Monitor::all()?;
    if let Some(m) = monitors.into_iter().nth(index) {
        Ok(m)
    } else {
        Monitor::all()?
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .ok_or_else(|| anyhow::anyhow!("no primary monitor found"))
    }
}
