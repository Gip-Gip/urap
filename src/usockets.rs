//! Primary and Secondary client and server for use with Unix Sockets. 

use crate::{
    Error, StdIo, UrapPrimary as UrapPrimaryProto, UrapSecondary as UrapSecondaryProto, Read, Write,
    URAP_DATA_WIDTH, URAP_HEAD_WIDTH, URAP_REG_WIDTH, URAP_COUNT_MAX, URAP_CRC_WIDTH, NakCode,
};
use std::{
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    vec::Vec,
};

pub struct UrapSecondary {
    pub errors: Arc<Mutex<Vec<Error<std::io::Error>>>>,
    pub join_handle: JoinHandle<Result<(), std::io::Error>>,
}

impl UrapSecondary {
    pub fn spawn<const REGCNT: usize>(
        path: &str,
        registers: Arc<Mutex<[[u8; URAP_DATA_WIDTH]; REGCNT]>>,
        writeprotect: [bool; REGCNT],
    ) -> Result<Self, Error<std::io::Error>> {
        let listener = UnixListener::bind(path)?;

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
                            let mut stream: StdIo<UnixStream> = stream.into();
 
                            let mut urap_secondary = UrapSecondaryProto::new(
                                &mut stream,
                                &writeprotect,
                            );

                            loop {
                                let result = urap_secondary.poll();

                                let mut errors = error_cpy.lock().unwrap();

                                if let Err(e) = result {
                                    errors.push(e);
                                    // Terminate the connection if there's an error, to prevent
                                    // either side from hanging
                                    stream
                                        .get_inner_mut()
                                        .shutdown(Shutdown::Both)
                                        .unwrap_or_default();

                                    drop(errors);
                                    break;
                                } else if let Ok(result) = result {
                                    if let Some(packet) = result {

                                        let nak_code = packet.nak_code.clone();

                                        if let Some(nak_code) = nak_code {
                                            let e = match nak_code {
                                                NakCode::SecondaryFailure => Error::SecondaryFailure,
                                                NakCode::BadCrc => Error::BadCrc,
                                                NakCode::OutOfBounds => Error::OutOfBounds(packet.start_register),
                                                NakCode::IncompletePacket => Error::IncompletePacket,
                                                NakCode::IndexWriteProtected => Error::IndexWriteProtected(packet.count, packet.start_register),
                                                NakCode::CountExceedsBounds => Error::CountExceedsBounds(packet.count, packet.start_register),
                                                NakCode::Unknown => panic!("Unknown NAK code!"),
                                            };

                                            errors.push(e);
                                        }

                                        let mut registers = regcopy.lock().unwrap();
                                        let result = urap_secondary.process(packet, &mut registers);
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

                                        if nak_code.is_some() {
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

                                        drop(registers)
                                    }
                                }
    
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
        let socket = UnixStream::connect(path)?;
        socket.set_nonblocking(false).unwrap();

        let socket = socket.into();

        Ok(Self { socket })
    }

    #[inline]
    pub fn read_4u8(&mut self, register: u16, buffer: &mut [[u8; URAP_DATA_WIDTH]]) -> Result<(), Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).read_4u8(register, buffer)
    }

    //#[inline]
    //pub fn read_f32(&mut self, register: u16) -> Result<f32, Error<std::io::Error>> {
    //    UrapPrimaryProto::new(&mut self.socket).read_f32(register)
    //}

    //#[inline]
    //pub fn read_u32(&mut self, register: u16) -> Result<u32, Error<std::io::Error>> {
    //    UrapPrimaryProto::new(&mut self.socket).read_u32(register)
    //}

    //#[inline]
    //pub fn read_i32(&mut self, register: u16) -> Result<i32, Error<std::io::Error>> {
    //    UrapPrimaryProto::new(&mut self.socket).read_i32(register)
    //}

    #[inline]
    pub fn write_4u8(
        &mut self,
        start_register: u16,
        data: &[[u8; 4]],
    ) -> Result<(), Error<std::io::Error>> {
        UrapPrimaryProto::new(&mut self.socket).write_4u8(start_register, data)
    }

    //#[inline]
    //pub fn write_f32(&mut self, register: u16, num: f32) -> Result<(), Error<std::io::Error>> {
    //    UrapPrimaryProto::new(&mut self.socket).write_f32(register, num)
    //}

    //#[inline]
    //pub fn write_u32(&mut self, register: u16, num: u32) -> Result<(), Error<std::io::Error>> {
    //    UrapPrimaryProto::new(&mut self.socket).write_u32(register, num)
    //}

    //#[inline]
    //pub fn write_i32(&mut self, register: u16, num: i32) -> Result<(), Error<std::io::Error>> {
    //    UrapPrimaryProto::new(&mut self.socket).write_i32(register, num)
    //}

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
        const RCOUNT: usize = u16::MAX as usize + 1;
        let registers = Arc::new(Mutex::new([[0u8; URAP_DATA_WIDTH]; RCOUNT]));

        let mut write_protect: [bool; RCOUNT] = [false; RCOUNT];

        write_protect[2] = true;

        let secondary_path = Path::new(SLAVE_PATH);

        if secondary_path.exists() {
            remove_file(secondary_path).unwrap();
        }

        let mut urap_secondary =
            UrapSecondary::spawn(SLAVE_PATH, registers.clone(), write_protect).unwrap();

        let mut urap_primary = UrapPrimary::new(SLAVE_PATH).unwrap();

        assert!(urap_primary.is_healthy());

        for error in urap_secondary.errors.lock().unwrap().iter() {
            panic!("{}", error);
        }

        let mut buffer: [[u8; URAP_DATA_WIDTH]; 3] = [[0; URAP_DATA_WIDTH]; 3];

        urap_primary.read_4u8(0, &mut buffer).unwrap();

        urap_primary.write_4u8(0, &[
            f32::INFINITY.to_le_bytes(),
            42_u32.to_le_bytes(),
        ]).unwrap();
        
        urap_primary.write_4u8(2, &[
            (-1_i32).to_le_bytes(),
        ]).unwrap_err();

        let error = urap_secondary.pop_error().unwrap();
        match error {
            Error::IndexWriteProtected(_, _) => {}
            _ => {
                panic!("Incorrect Error Returned! {}", error)
            }
        }

        let mut urap_primary = UrapPrimary::new(SLAVE_PATH).unwrap();

        urap_primary.write_4u8(u16::MAX, &mut buffer).unwrap_err();

        let error = urap_secondary.pop_error().unwrap();
        match error {
            Error::CountExceedsBounds(_, _) => {}
            _ => {
                panic!("Incorrect Error Returned! {}", error)
            }
        }
        
        let mut urap_primary = UrapPrimary::new(SLAVE_PATH).unwrap();
        
        urap_primary.write_4u8(u16::MAX, &[f32::INFINITY.to_le_bytes()]).unwrap();
       
        let mut registers = registers.lock().unwrap();

        assert_eq!(registers[0], f32::INFINITY.to_le_bytes());
        assert_eq!(registers[1], 42_u32.to_le_bytes());
        assert_eq!(registers[2], 0_i32.to_le_bytes());

        assert_eq!(registers[u16::MAX as usize], f32::INFINITY.to_le_bytes());

        registers[2] = (-1_i32).to_le_bytes();
        drop(registers);

        urap_primary.read_4u8(0, &mut buffer).unwrap();
        
        assert_eq!(f32::from_le_bytes(buffer[0]), f32::INFINITY);
        assert_eq!(u32::from_le_bytes(buffer[1]), 42);
        assert_eq!(i32::from_le_bytes(buffer[2]), -1);
       
        drop(urap_secondary);

        if secondary_path.exists() {
            remove_file(secondary_path).unwrap();
        }
    }
}
