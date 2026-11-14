// Modified in the desktop-recorder-mcp fork of agent-desktop (see NOTICE).
use agent_desktop_core::{AdapterError, Deadline, ProcessIdentity};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows_sys::Win32::Networking::WinSock::AF_INET;

const LOOPBACK: u32 = 0x7f00_0001;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

pub(crate) fn owns_listener(
    process: &ProcessIdentity,
    port: u16,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if !crate::system::process_identity::matches_instance(process.pid, &process.instance)? {
        return Ok(false);
    }
    if deadline.is_expired() {
        return Err(AdapterError::timeout(
            "Checking the DevTools listener owner",
        ));
    }
    Ok(loopback_listener_rows()?
        .iter()
        .any(|(listening, owner)| *listening == port && *owner == u32::from(process.pid)))
}

/// The loopback TCP ports process `pid` listens on (a browser's DevTools
/// endpoint is one of them when it was started with one).
pub(crate) fn loopback_listeners(pid: u32) -> Vec<u16> {
    loopback_listener_rows()
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, owner)| *owner == pid)
        .map(|(port, _)| port)
        .collect()
}

/// Every IPv4 loopback listener as (port, owning process). The system's
/// listener table is asked for directly: in the table of all connections
/// some Windows builds report a listener's state as 0 instead of
/// `MIB_TCP_STATE_LISTEN`, so filtering that table by state finds nothing.
fn loopback_listener_rows() -> Result<Vec<(u16, u32)>, AdapterError> {
    let mut size = 0;
    let sized = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            AF_INET as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if sized != ERROR_INSUFFICIENT_BUFFER || size < std::mem::size_of::<u32>() as u32 {
        return Err(AdapterError::internal(
            "Windows could not size the TCP listener table",
        ));
    }
    // Listeners can appear between the two calls: leave room for a few more.
    size += 16 * std::mem::size_of::<MIB_TCPROW_OWNER_PID>() as u32;
    let mut buffer = vec![0_u32; size.div_ceil(4) as usize];
    let read = unsafe {
        GetExtendedTcpTable(
            buffer.as_mut_ptr().cast(),
            &mut size,
            0,
            AF_INET as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if read != 0 {
        return Err(AdapterError::internal(
            "Windows could not read the TCP listener table",
        ));
    }
    let table = unsafe { &*buffer.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>() };
    let rows =
        unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };
    Ok(rows
        .iter()
        .filter(|row: &&MIB_TCPROW_OWNER_PID| u32::from_be(row.dwLocalAddr) == LOOPBACK)
        .map(|row| (u16::from_be(row.dwLocalPort as u16), row.dwOwningPid))
        .collect())
}

#[cfg(test)]
#[path = "tcp_listener_tests.rs"]
mod tests;
