use super::*;

struct BrokenFlush;
impl Write for BrokenFlush {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn a_binary_output_flush_failure_is_returned_to_the_proxy_owner() {
    let error = copy_flushed(&mut [0, 1, 2, 3].as_slice(), &mut BrokenFlush).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
}

#[test]
fn interrupted_reads_do_not_truncate_binary_output() {
    let packet = [0, 1, 2, 3];
    let mut reader = InterruptedOnce {
        bytes: &packet,
        interrupted: false,
    };
    let mut output = Vec::new();
    copy_flushed(&mut reader, &mut output).unwrap();
    assert_eq!(output, packet);
}

struct InterruptedOnce<'a> {
    bytes: &'a [u8],
    interrupted: bool,
}

impl Read for InterruptedOnce<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if !self.interrupted {
            self.interrupted = true;
            return Err(io::ErrorKind::Interrupted.into());
        }
        self.bytes.read(buffer)
    }
}
