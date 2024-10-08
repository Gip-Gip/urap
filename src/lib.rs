#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]


#[cfg(feature = "usockets")]
#[cfg_attr(docsrs, doc(cfg(feature = "usockets")))]
pub mod usockets;

use core::fmt::Display;

use bytemuck::{bytes_of, cast_slice_mut, checked::{cast_slice, from_bytes}};
use embedded_io::{Read, ReadExactError, Write};
#[cfg(feature = "std")]
use embedded_io::ErrorType;

/// Number of bytes in a register
pub const URAP_DATA_WIDTH: usize = 4;
/// Number of bytes in a CRC
pub const URAP_CRC_WIDTH: usize = 1;
/// Number of bytes when addressing a register via URAP
pub const URAP_REG_WIDTH: usize = 2;
/// Number of bytes in a count
pub const URAP_COUNT_WIDTH: usize = 1;
/// Number of bytes in a head byte
pub const URAP_HEAD_WIDTH: usize = URAP_COUNT_WIDTH;
/// Number of bytes in an ACK
pub const URAP_ACK_WIDTH: usize = 1;
/// Most significant bit signifying a write in URAP
pub const URAP_WRITE_OR: u8 = 0x80;
/// Maximum register that can be accessed in a single packet
pub const URAP_COUNT_MAX: usize = 128;
/// Maximum amount of data in a packet
pub const URAP_MAX_DATA_SIZE: usize = URAP_DATA_WIDTH * URAP_COUNT_MAX;
/// Maximum size of a single packet
pub const URAP_MAX_PACKET_SIZE: usize = URAP_HEAD_WIDTH + URAP_REG_WIDTH + URAP_DATA_WIDTH * URAP_COUNT_MAX + URAP_CRC_WIDTH;

/// CRC Table for polynomial 0x1D
pub static CRC_TABLE: [u8; 256] = [
 0x00, 0x1D, 0x3A, 0x27, 0x74, 0x69, 0x4E, 0x53, 0xE8, 0xF5, 0xD2, 0xCF, 0x9C, 0x81, 0xA6, 0xBB, 
 0xCD, 0xD0, 0xF7, 0xEA, 0xB9, 0xA4, 0x83, 0x9E, 0x25, 0x38, 0x1F, 0x02, 0x51, 0x4C, 0x6B, 0x76, 
 0x87, 0x9A, 0xBD, 0xA0, 0xF3, 0xEE, 0xC9, 0xD4, 0x6F, 0x72, 0x55, 0x48, 0x1B, 0x06, 0x21, 0x3C, 
 0x4A, 0x57, 0x70, 0x6D, 0x3E, 0x23, 0x04, 0x19, 0xA2, 0xBF, 0x98, 0x85, 0xD6, 0xCB, 0xEC, 0xF1, 
 0x13, 0x0E, 0x29, 0x34, 0x67, 0x7A, 0x5D, 0x40, 0xFB, 0xE6, 0xC1, 0xDC, 0x8F, 0x92, 0xB5, 0xA8, 
 0xDE, 0xC3, 0xE4, 0xF9, 0xAA, 0xB7, 0x90, 0x8D, 0x36, 0x2B, 0x0C, 0x11, 0x42, 0x5F, 0x78, 0x65, 
 0x94, 0x89, 0xAE, 0xB3, 0xE0, 0xFD, 0xDA, 0xC7, 0x7C, 0x61, 0x46, 0x5B, 0x08, 0x15, 0x32, 0x2F, 
 0x59, 0x44, 0x63, 0x7E, 0x2D, 0x30, 0x17, 0x0A, 0xB1, 0xAC, 0x8B, 0x96, 0xC5, 0xD8, 0xFF, 0xE2, 
 0x26, 0x3B, 0x1C, 0x01, 0x52, 0x4F, 0x68, 0x75, 0xCE, 0xD3, 0xF4, 0xE9, 0xBA, 0xA7, 0x80, 0x9D, 
 0xEB, 0xF6, 0xD1, 0xCC, 0x9F, 0x82, 0xA5, 0xB8, 0x03, 0x1E, 0x39, 0x24, 0x77, 0x6A, 0x4D, 0x50, 
 0xA1, 0xBC, 0x9B, 0x86, 0xD5, 0xC8, 0xEF, 0xF2, 0x49, 0x54, 0x73, 0x6E, 0x3D, 0x20, 0x07, 0x1A, 
 0x6C, 0x71, 0x56, 0x4B, 0x18, 0x05, 0x22, 0x3F, 0x84, 0x99, 0xBE, 0xA3, 0xF0, 0xED, 0xCA, 0xD7, 
 0x35, 0x28, 0x0F, 0x12, 0x41, 0x5C, 0x7B, 0x66, 0xDD, 0xC0, 0xE7, 0xFA, 0xA9, 0xB4, 0x93, 0x8E, 
 0xF8, 0xE5, 0xC2, 0xDF, 0x8C, 0x91, 0xB6, 0xAB, 0x10, 0x0D, 0x2A, 0x37, 0x64, 0x79, 0x5E, 0x43, 
 0xB2, 0xAF, 0x88, 0x95, 0xC6, 0xDB, 0xFC, 0xE1, 0x5A, 0x47, 0x60, 0x7D, 0x2E, 0x33, 0x14, 0x09, 
 0x7F, 0x62, 0x45, 0x58, 0x0B, 0x16, 0x31, 0x2C, 0x97, 0x8A, 0xAD, 0xB0, 0xE3, 0xFE, 0xD9, 0xC4, 
];

/// Calculate the CRC of given data using the table CRC_TABLE
pub fn crc(start_crc: u8, data: &[u8]) -> u8 {
    let mut crc = start_crc;

    for byte in data {
        crc = CRC_TABLE[(*byte ^ crc) as usize];
    }

    crc
}

/// ACK byte, set to 0xAA due to it's resiliance to most natural interference
pub const ACK: u8 = 0xAA;

#[repr(u8)]
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
/// Possible NAK codes, see README for more info
pub enum NakCode {
    Unknown = 0,
    SecondaryFailure = 1,
    BadCrc = 2,
    OutOfBounds = 3,
    IncompletePacket = 4,
    IndexWriteProtected = 5,
    CountExceedsBounds = 6,
}

impl From<u8> for NakCode {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::SecondaryFailure,
            2 => Self::BadCrc,
            3 => Self::OutOfBounds,
            4 => Self::IncompletePacket,
            5 => Self::IndexWriteProtected,
            6 => Self::CountExceedsBounds,
            _ => Self::Unknown,
        }
    }
}

/// Errors a Primary client or Secondary server can return
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub enum Error<E> {
    Io(E),
    Nak(NakCode),
    SecondaryFailure,
    BadCrc,
    OutOfBounds(u16),
    IncompletePacket,
    IndexWriteProtected(u8, u16),
    CountExceedsBounds(u8, u16),
}

impl<E> Display for Error<E>
where
    E: Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{}", e),
            Error::Nak(code) => write!(f, "NAK Recieved, code {:?}", code),
            Error::SecondaryFailure => write!(f, "Secondary Failure"),
            Error::BadCrc => write!(f, "Bad Crc"),
            Error::OutOfBounds(index) => write!(
                f,
                "Attempted to access index {}, which is out of bounds",
                index
            ),
            Error::IncompletePacket => write!(f, "Incomplete Packet"),
            Error::IndexWriteProtected(count, index) => write!(
                f,
                "Attempted to write to a write protected index between {} and {}",
                index,
                index + *count as u16
            ),
            Error::CountExceedsBounds(count, index) => write!(
                f,
                "Attempted to access {} registers at index {}, which exceeds bounds",
                count,
                index,
            ),
        }
    }
}

impl<E> From<E> for Error<E> {
    fn from(value: E) -> Self {
        Error::Io(value)
    }
}

impl<E> From<ReadExactError<E>> for Error<E> {
    fn from(value: ReadExactError<E>) -> Self {
        match value {
            ReadExactError::UnexpectedEof => Error::IncompletePacket,
            ReadExactError::Other(e) => Error::Io(e),
        }
    }
}


// A bunch of conversions that make this compatible between both embedded_io
// and std::io. Why there still isn't a core::io, you tell me.
#[cfg(feature = "std")]
pub struct StdIo<IO>
where
    IO: std::io::Read + std::io::Write,
{
    io: IO,
}

#[cfg(feature = "std")]
impl<IO> StdIo<IO>
where
    IO: std::io::Read + std::io::Write,
{
    #[inline]
    pub fn get_inner(&mut self) -> &IO {
        &self.io
    }

    #[inline]
    pub fn get_inner_mut(&mut self) -> &mut IO {
        &mut self.io
    }
}

#[cfg(feature = "std")]
impl<IO> From<IO> for StdIo<IO>
where
    IO: std::io::Read + std::io::Write,
{
    #[inline]
    fn from(value: IO) -> Self {
        Self { io: value }
    }
}

#[cfg(feature = "std")]
impl<IO> ErrorType for StdIo<IO>
where
    IO: std::io::Read + std::io::Write,
{
    type Error = std::io::Error;
}

#[cfg(feature = "std")]
impl<IO> Read for StdIo<IO>
where
    IO: std::io::Read + std::io::Write,
{
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.io.read(buf)
    }
}

#[cfg(feature = "std")]
impl<IO> Write for StdIo<IO>
where
    IO: std::io::Read + std::io::Write,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.io.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.io.flush()
    }
}

/// Secondary server struct, allows you to poll and process incoming packets.
pub struct UrapSecondary<'a, 'c, IO, const REGCNT: usize>
where
    IO: Read + Write,
{
    io: &'a mut IO,
    writeprotect: &'c [bool; REGCNT],
}

impl<'a, 'c, IO, const REGCNT: usize> UrapSecondary<'a, 'c, IO, REGCNT>
where
    IO: Read + Write,
{
    /// Create a new secondary server with IO and a slice with boolean values
    /// corresponding to the write protect status of individual registers.
    pub fn new(
        io: &'a mut IO,
        writeprotect: &'c [bool; REGCNT],
    ) -> Self {
        Self {
            io,
            writeprotect,
        }
    }

    /// Poll the IO for data, and if there is data return the recieved packet
    /// to be further processed.
    pub fn poll(&mut self) -> Result<Option<UrapRecievedPacket>, Error<IO::Error>> {
        let mut buffer: [u8; URAP_HEAD_WIDTH + URAP_REG_WIDTH] = [0; URAP_HEAD_WIDTH + URAP_REG_WIDTH];

        let i = self.io.read(&mut buffer)?;

        if i > 0 {
            if i < buffer.len() {
                let buffer_len = buffer.len();
                self.io.read_exact(&mut buffer[i..buffer_len])?;
            }
        
            let head = buffer[0];
            let write = head & URAP_WRITE_OR > 0;
            let count = (head & !URAP_WRITE_OR) + 1;
            let calcd_crc = crc(0, &buffer);

            let start_register = u16::from_le_bytes([buffer[1], buffer[2]]);

            if write {
                let mut buffer: [u8; URAP_MAX_DATA_SIZE + URAP_CRC_WIDTH] = [0; URAP_MAX_DATA_SIZE + URAP_CRC_WIDTH];

                let count_bytes = count as usize * URAP_DATA_WIDTH;

                self.io.read_exact(&mut buffer[..count_bytes + URAP_CRC_WIDTH])?;

                let calcd_crc = crc(calcd_crc, &buffer[..count_bytes + URAP_CRC_WIDTH]);

                let nak_code = if calcd_crc != 0 {
                    Some(NakCode::BadCrc)
                } else if start_register as usize >= REGCNT {
                    Some(NakCode::OutOfBounds)
                } else if start_register as usize + count as usize > REGCNT {
                    Some(NakCode::CountExceedsBounds)
                } else {
                    let mut write_protected = false;
                    
                    for reg in &self.writeprotect[start_register as usize..start_register as usize + count as usize] {
                        write_protected = write_protected || *reg;
                    }

                    if write_protected {
                        Some(NakCode::IndexWriteProtected)
                    } else {
                        None
                    }
                };

                let write_buffer: &[[u8; URAP_DATA_WIDTH]; URAP_COUNT_MAX] = from_bytes(&buffer[..URAP_MAX_DATA_SIZE]);

                Ok(Some(UrapRecievedPacket {
                    count,
                    start_register,
                    write_buffer: Some(*write_buffer),
                    nak_code,
                }))
            } else {
                let mut buffer: [u8; URAP_CRC_WIDTH] = [0; URAP_CRC_WIDTH];

                self.io.read_exact(&mut buffer)?;

                let calcd_crc = crc(calcd_crc, &buffer[..URAP_CRC_WIDTH]);

                let nak_code = if calcd_crc != 0 {
                    Some(NakCode::BadCrc)
                } else if start_register as usize >= REGCNT {
                    Some(NakCode::OutOfBounds)
                } else if start_register as usize + count as usize > REGCNT {
                    Some(NakCode::CountExceedsBounds)
                } else {
                    None
                };

                Ok(Some(UrapRecievedPacket{
                    count,
                    start_register,
                    write_buffer: None,
                    nak_code,
                }))
            }
        } else {
            Ok(None)
        }
    }

    /// Process a packet read by polling.
    pub fn process(&mut self, recieved_packet: UrapRecievedPacket, registers: &mut [[u8; URAP_DATA_WIDTH]; REGCNT]) -> Result<(), Error<IO::Error>> {
        if let Some(nak_code) = recieved_packet.nak_code {
            self.io.write_all(&[nak_code as u8])?;

            return Ok(());
        }

        let start_register = recieved_packet.start_register as usize;
        let end_register = recieved_packet.start_register as usize + recieved_packet.count as usize;

        if let Some(write_buffer) = recieved_packet.write_buffer {
            registers[start_register..end_register].copy_from_slice(&write_buffer[..recieved_packet.count as usize]);

            self.io.write_all(&[ACK])?;
        } else {
            let mut buffer: [u8; URAP_ACK_WIDTH + URAP_MAX_DATA_SIZE + URAP_CRC_WIDTH] = [ACK; URAP_ACK_WIDTH + URAP_MAX_DATA_SIZE + URAP_CRC_WIDTH];

            let reg_start_offset = URAP_ACK_WIDTH;
            let reg_end_offset = reg_start_offset + URAP_DATA_WIDTH * recieved_packet.count as usize;
            let crc_index = reg_end_offset;
            let buffer_len = reg_end_offset + URAP_CRC_WIDTH;

            buffer[reg_start_offset..reg_end_offset].copy_from_slice(cast_slice(&registers[start_register..end_register]));

            let calcd_crc = crc(0, &buffer[reg_start_offset..reg_end_offset]);

            buffer[crc_index] = calcd_crc;

            self.io.write_all(&buffer[..buffer_len])?;
        }

        Ok(())
    }
}

/// A packet recieved during polling.
pub struct UrapRecievedPacket {
    /// Number of registers being accessed during operation.
    pub count: u8,
    /// The first register to be accessed in operation
    pub start_register: u16,
    /// If None, this is a read operation. If Some, the Some contains the entire write buffer.
    pub write_buffer: Option<[[u8; URAP_DATA_WIDTH]; URAP_COUNT_MAX]>,
    /// If there was an error the Nak code is here; needs to be written to the Primary first.
    pub nak_code: Option<NakCode>,
}

/// Primary client, used for interacting with a server via IO.
pub struct UrapPrimary<'a, IO>
where
    IO: Read + Write,
{
    io: &'a mut IO,
}

impl<'a, IO> UrapPrimary<'a, IO>
where
    IO: Read + Write,
{
    /// Create a client with IO.
    pub fn new(io: &'a mut IO) -> Self {
        Self { io }
    }

    /// Read `n` registers into an array of `[[u8; 4]; n]`
    pub fn read_4u8(&mut self, start_register: u16, data: &mut [[u8; 4]]) -> Result<(), Error<IO::Error>> {
        assert!(data.len() <= URAP_COUNT_MAX);

        if data.len() == 0 {
            return Ok(());
        }

        let start_register = start_register.to_le_bytes();

        let count = (data.len() - 1) as u8;

        let calcd_crc = crc(0, &[count]);
        let calcd_crc = crc(calcd_crc, &start_register);

        let packet_data: [u8; URAP_COUNT_WIDTH + URAP_REG_WIDTH + URAP_CRC_WIDTH] = [
            count,
            start_register[0],
            start_register[1],
            calcd_crc
        ];

        self.io.write_all(&packet_data)?;

        let mut ack_or_nak: [u8; 1] = [0; 1];

        self.io.read_exact(&mut ack_or_nak)?;


        if ack_or_nak[0] != ACK {
            return Err(Error::Nak(ack_or_nak[0].into()));
        }

        let data_bytes: &mut [u8] = cast_slice_mut(data);

        self.io.read_exact(data_bytes)?;

        let calcd_crc = crc(0, &data_bytes);

        let mut crc_data: [u8; URAP_CRC_WIDTH] = [0; URAP_CRC_WIDTH];

        self.io.read_exact(&mut crc_data)?;

        if crc(calcd_crc, &crc_data) != 0 {
            return Err(Error::BadCrc);
        }

        Ok(())
    }
 
    /// Write `n` registers from an array of `[[u8; 4]; n]`
    pub fn write_4u8(&mut self, start_register: u16, data: &[[u8; URAP_DATA_WIDTH]]) -> Result<(), Error<IO::Error>> {
        assert!(data.len() <= URAP_COUNT_MAX);

        if data.len() == 0 {
            return Ok(());
        }

        let start_register = start_register.to_le_bytes();

        let count = (data.len() - 1) as u8;
        let head = count | URAP_WRITE_OR;
        let data_bytes: &[u8] = cast_slice(data);

        let mut packet_data: [u8; URAP_MAX_PACKET_SIZE] = [0; URAP_MAX_PACKET_SIZE];

        let data_start_index = URAP_HEAD_WIDTH + URAP_REG_WIDTH;
        let data_end_index = data_start_index + data_bytes.len();
        let crc_index = data_end_index;
        let packet_end_index = crc_index + 1;

        packet_data[0] = head;
        packet_data[1] = start_register[0];
        packet_data[2] = start_register[1];
        packet_data[data_start_index..data_end_index].copy_from_slice(data_bytes);

        let calcd_crc = crc(0, &packet_data[..crc_index]);
        packet_data[crc_index] = calcd_crc;

        self.io.write_all(&packet_data[..packet_end_index])?;

        let mut ack_or_nak: [u8; 1] = [0];

        self.io.read_exact(&mut ack_or_nak)?;

        if ack_or_nak[0] != ACK {
            return Err(Error::Nak(ack_or_nak[0].into()));
        }

        Ok(())
    }

    /// Check if the connection is healthy
    #[inline]
    pub fn is_healthy(&mut self) -> bool {
        let mut buffer: [[u8; URAP_DATA_WIDTH]; 1] = [[0; URAP_DATA_WIDTH]; 1];
        match self.read_4u8(0, &mut buffer) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}
