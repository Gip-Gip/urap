use crate::{
    Error, StdIo, UrapPrimary as UrapPrimaryProto, UrapSecondary as UrapSecondaryProto,
    URAP_DATA_WIDTH,
};
use std::{
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

pub struct UrapSecondary {
    pub errors: Arc<Mutex<Vec<Error<std::io::Error>>>>,
    pub join_handle: JoinHandle<Result<(), std::io::Error>>,
}

impl UrapSecondary {
    pub fn spawn<const REGCNT: usize>(
        address: &str,
        registers: Arc<Mutex<[[u8; URAP_DATA_WIDTH]; REGCNT]>>,
        writeprotect: [bool; REGCNT],
    ) -> Result<Self, Error<std::io::Error>> {
        let listener = UnixListener::bind(address)?;

        let errors: Arc<Mutex<Vec<Error<std::io::Error>>>> = Arc::new(Mutex::new(Vec::new()));

        let error_cpy = errors.clone();

        let join_handle = thread::spawn(move || loop {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let regcopy = registers.clone();
                        let error_cpy = error_cpy.clone();
                        stream.set_nonblocking(false).unwrap();
                        thread::spawn(move || {
                            let mut buffer: [u8; 1] = [0; 1];
                            let mut stream: StdIo<UnixStream> = stream.into();
                            while stream.get_inner_mut().peek(&mut buffer).unwrap_or(0) != 0 {
                                let mut registers = regcopy.lock().unwrap();
                                let mut errors = error_cpy.lock().unwrap();
                                let mut urap_secondary = UrapSecondaryProto::new(
                                    &mut stream,
                                    &mut registers,
                                    &writeprotect,
                                );

                                let result = urap_secondary.poll();

                                if let Err(e) = result {
                                    errors.push(e);
                                    // Terminate the connection if there's an error, to prevent
                                    // either side from hanging
                                    stream
                                        .get_inner_mut()
                                        .shutdown(Shutdown::Both)
                                        .unwrap_or_default();

                                    drop(registers);
                                    drop(errors);
                                    break;
                                }

                                drop(registers);
                                drop(errors);
                            }
                        });
                    }
                    Err(_) => {}
                }
            }
        });

        Ok(Self {
            errors,
            join_handle,
        })
    }

    pub fn pop_error(&mut self) -> Option<Error<std::io::Error>> {
        let mut errors = self.errors.lock().unwrap();

        let error = errors.pop();

        drop(errors);

        error
    }
}

pub struct UrapPrimary {
    socket: StdIo<UnixStream>,
}

impl UrapPrimary {
    pub fn new(path: &str) -> Result<Self, std::io::Error> {
        let socket = UnixStream::connect(path)?.into();

        Ok(Self { socket })
    }

    #[inline]
    pub fn read_4u8(&mut self, register: u16) -> Result<[u8; 4], Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).read_4u8(register)
    }

    #[inline]
    pub fn read_f32(&mut self, register: u16) -> Result<f32, Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).read_f32(register)
    }

    #[inline]
    pub fn read_u32(&mut self, register: u16) -> Result<u32, Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).read_u32(register)
    }

    #[inline]
    pub fn read_i32(&mut self, register: u16) -> Result<i32, Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).read_i32(register)
    }

    #[inline]
    pub fn write_4u8(
        &mut self,
        register: u16,
        buffer: &[u8; 4],
    ) -> Result<(), Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).write_4u8(register, buffer)
    }

    #[inline]
    pub fn write_f32(&mut self, register: u16, num: f32) -> Result<(), Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).write_f32(register, num)
    }

    #[inline]
    pub fn write_u32(&mut self, register: u16, num: u32) -> Result<(), Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).write_u32(register, num)
    }

    #[inline]
    pub fn write_i32(&mut self, register: u16, num: i32) -> Result<(), Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).write_i32(register, num)
    }

    #[inline]
    pub fn is_healthy(&mut self) -> bool {
        UrapPrimaryProto::new(&mut self.socket).is_healthy()
    }
}

impl Drop for UrapPrimary {
    fn drop(&mut self) {
        self.socket
            .get_inner_mut()
            .shutdown(Shutdown::Both)
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::remove_file, path::Path};

    use super::*;

    static SLAVE_PATH: &str = "test.socket";

    #[test]
    fn unix_sockets() {
        let registers = Arc::new(Mutex::new([[0u8; URAP_DATA_WIDTH]; 3]));

        let secondary_path = Path::new(SLAVE_PATH);

        if secondary_path.exists() {
            remove_file(secondary_path).unwrap();
        }

        let mut urap_secondary =
            UrapSecondary::spawn(SLAVE_PATH, registers.clone(), [false, false, true]).unwrap();

        let mut urap_primary = UrapPrimary::new(SLAVE_PATH).unwrap();

        assert!(urap_primary.is_healthy());

        assert_eq!(urap_primary.read_f32(0).unwrap(), 0.0);
        assert_eq!(urap_primary.read_u32(1).unwrap(), 0);
        assert_eq!(urap_primary.read_i32(2).unwrap(), 0);

        urap_primary.write_f32(0, f32::INFINITY).unwrap();
        urap_primary.write_u32(1, 42).unwrap();
        urap_primary.write_i32(2, -46).unwrap_err();

        let error = urap_secondary.pop_error().unwrap();
        match error {
            Error::IndexWriteProtected(_) => {}
            _ => {
                panic!("Incorrect Error Returned! {}", error)
            }
        }

        let registers = registers.lock().unwrap();

        assert_eq!(registers[0], f32::INFINITY.to_le_bytes());
        assert_eq!(registers[1], 42_u32.to_le_bytes());
        assert_eq!(registers[2], 0_i32.to_le_bytes());

        drop(registers);

        assert_eq!(urap_primary.is_healthy(), false);

        let mut urap_primary = UrapPrimary::new(SLAVE_PATH).unwrap();

        assert_eq!(urap_primary.read_f32(0).unwrap(), f32::INFINITY);
        assert_eq!(urap_primary.read_u32(1).unwrap(), 42);
        assert_eq!(urap_primary.read_i32(2).unwrap(), 0);

        drop(urap_secondary);

        if secondary_path.exists() {
            remove_file(secondary_path).unwrap();
        }
    }
}
