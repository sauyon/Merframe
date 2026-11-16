#[derive(Debug, thiserror::Error)]
pub enum MemError {
    #[error("Process {0} not found")]
    ProcessNotFound(String),
    #[error("Module {0} not mapped")]
    ModuleNotFound(String),
    #[cfg_attr(
        not(windows),
        error(
            "Reading process {0} is not permitted, set kernel.yama.ptrace_scope to 0 or grant Merframe CAP_SYS_PTRACE"
        )
    )]
    #[cfg_attr(
        windows,
        error("Reading process {0} is not permitted, run Merframe as administrator like the game")
    )]
    AccessDenied(u32),
    #[error("Read at {addr:#x} ({len} {}) failed: {source}", if *len == 1 { "byte" } else { "bytes" })]
    Read {
        addr: u64,
        len: usize,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, MemError>;

#[cfg(test)]
mod tests {
    use super::MemError;

    #[test]
    fn read_byte_count() {
        for (len, unit) in [(0, "bytes"), (1, "byte"), (2, "bytes")] {
            let error = MemError::Read {
                addr: 0x10,
                len,
                source: std::io::Error::other("unreadable"),
            };
            assert_eq!(
                error.to_string(),
                format!("Read at 0x10 ({len} {unit}) failed: unreadable")
            );
        }
    }
}
