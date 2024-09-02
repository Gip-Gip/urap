*A simple protocol for accessing data across various transmission means.*

The Universal Register Access Protocol, or URAP for short, is a lightweight,
fast and simple protocol for accessing registers through a variety of data
transmission standards, be it Unix Sockets, UART, RS-485, Ethernet IP or USB
Serial, URAP is simple to implement and open to be used by all.

## Protocol Overview
URAP is a data communication protocol where a primary client reads and writes
data to a secondary server, which allows access to up to 2^16 registers, each
32 bits wide. It assumes that it is being trasmitted on a one to one stream
where no other possible processes or devices can listen in or interpret the
packets; in other words when communicating over multi-device RS-485 or Ethernet
another transport protocol is required such as IPv4. It is not secure by any
means nor is it intended to be.

There are 5 types of packets and they are all dead simple:

 * Read packet, which has the WRITE bit set to zero, contain the WRITE bit,
 the 7 bit register count, the 16 bit starting register address, and a CRC
 * Write packet, which has the WRITE bit set to one, contains the WRITE bit,
 the 7 bit register count, the 16 bit register address, the data to write, and a CRC
 * Read ACK packet, contains an ACK byte, the data read, and a CRC
 * Write ACK packet, contains an ACK byte
 * NAK packet, containes a NAK byte

With these 5 packets you can have almost all the functionality you need for
simple embedded and non-embedded applications, where you just want to exchange
variables back and forth.

### Packet Structure

#### Read Packet

 2. **Bit 0**: Write bit, set to zero since we are reading
 1. **Bit 1-7**: Register count, the number of registers to be read -1
 3. **Bits 8-23**: First register to read(little endian)
 4. **Bits 24-31**: CRC of bits 0-23

#### Read-ACK Packet

 1. **Bits 0-7**: ACK byte, equal to 0xAA
 2. **Bits 8-(7 + count * 32)**: Data contained in the registers
 3. **Bits (7 + count * 32 + 1)-(7 + count * 32 + 8)**: CRC of bits 8-(7 + count * 32)

#### Write Packet

 2. **Bit 0**: Write bit, set to one since we are writing
 1. **Bit 1-7**: Register count, the number of registers to be read -1
 3. **Bits 8-23**: First register to write to
 4. **Bits 24-(23 + count * 32)**: Data to write to the register
 5. **Bits (23 + count * 32 + 1)-(23 + count * 32 + 8)**: CRC of bits 24-(23 + count * 32)

#### Write-ACK Packet

 1. **Bits 0-7**: ACK byte, equal to 0xAA

#### NAK Packet

 1. **Bits 0-7**: NAK code. Any byte with a value other than 0xAA is a NAK,
 however it is recommended you use the following codes to indicate to the
 primary what the failure is.

##### NAK Codes
 * **0x00**: Unknown, highly recommended to avoid sending this on purpose
 * **0x01**: SecondaryFailure, basically any computation error that is not due
 to transmission or primary fault
 * **0x02**: BadCrc, whenever there is a mismatch between the sent CRC and the
 computed CRC
 * **0x03**: OutOfBounds, when the primary attempts to access a register which
 doesn't exist on the secondary
 * **0x04**: IncompletePacket, when the packet sent to the secondary is missing
 something
 * **0x05**: IndexWriteProtected, whenever the secondary attempts to write to a
 write-protected register
 * **0x06**: CountExceedsBounds, when the first register the primary wants to
 access is in bounds, but a register to be accessed via the count is out of
 bounds

### Endianness

All integers, floats, and etc. past 8 bits are little endian, due to the
fact that all modern architectures are little endian. This includes register
addresses.

Note this makes the bit layout look unusual, but it forgoes having to do any
conversion on 99% of processors. This results in the write bit actually
being the 9th bit in the packet, but the highest bit when programming.

For example, the array `data` in the following code actually contains the
bytes `[0b0000_0000, 0b1000_0000]`.

```text
let register: u16 = 0b1000_0000_0000_0000;
let data: [u8; 2] = register.to_le_bytes();
```

This seems baffling when you look at the data being transferred, but to the
processor and the code written this is as simple as it gets.

### CRC

URAP uses an 8 bit CRC with 0x1D as it's polynomial, the same polynomial used
for OBD. It does not use any weird init values, it is not reflected, and it is
not XOR'd with anything post calculation. See
[this link](http://www.sunshine2k.de/articles/coding/crc/understanding_crc.html)
for more info on how it works.

It should be noted that ACK and NAK bytes are not CRC'd.

### Checking the health of a connection

To check the health of a connection, it is standard to read register 0 of the
secondary and await a healthy response. It is recommended that URAP secondaries
have minimum of 1 register due to this otherwise this check will not work. Also
ensure that **if you implement any form of read protection that register zero
remains readable.**

### Write protection

URAP Secondaries are encouraged to use write protection on registers which are
intended to be read-only, in case of the odd chance a primary sends an erronous
command. If there is an attempt to write to a write-protected register, the
secondary should respond with a NAK to indicate that no writes have been
committed and that there should be a change of code to fix this issue.

### Example transactions

#### Write to Register 0

Primary -> Secondary, write register zero with value 42
```text
1                                                                                   Write Bit
 000 0000                                                                           Count -1
          0000 0000 0000 0000                                                       Register
                              0010 1010 0000 0000 0000 0000 0000 0000               Value
                                                                      0000 1111     CRC
```

Secondary -> Primary, write-ack
```text
1010 1010   ACK
```

Alternatively, if register 0 is write protected you will get a NAK


Secondary -> Primary, nak

```text
0000 0101   NAK, IndexWriteProtected
```

#### Read Register 0

Primary -> Secondary, read register zero
```text
0                                           Write Bit
 000 0000                                   Count -1
          0000 0000 0000 0000               Register
                              0000 0000     CRC
```

Secondary -> Primary, read-ack with the value 42
```text
1010 1010                                                       ACK
          0010 1010 0000 0000 0000 0000 0000 0000               Value
                                                  0100 1111     CRC
```

## Legal

These specifications are under
[CC BY](https://creativecommons.org/licenses/by/4.0/). The source code in this
repository is under the MIT License, and you can see the LICENSE file for more
details.
