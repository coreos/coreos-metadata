/// Misc network-related helpers.
use crate::errors::*;
use std::io::Write;
use std::net::IpAddr;

/// Write nameservers in `resolv.conf` format.
pub(crate) fn write_resolvconf<T>(writer: &mut T, nameservers: &[IpAddr]) -> Result<()>
where
    T: Write,
{
    slog_scope::trace!("writing {} nameservers", nameservers.len());

    for ns in nameservers {
        let entry = format!("nameserver {}\n", ns);
        writer.write_all(&entry.as_bytes())?;
        writer.flush()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_resolvconf() {
        let nameservers = vec![IpAddr::from([4, 4, 4, 4]), IpAddr::from([8, 8, 8, 8])];
        let expected = "nameserver 4.4.4.4\nnameserver 8.8.8.8\n";
        let mut buf = vec![];

        write_resolvconf(&mut buf, &nameservers).unwrap();
        assert_eq!(buf, expected.as_bytes());
    }
}
