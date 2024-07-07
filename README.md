*A simple protocol for accessing data across various transmission means.*

The Universal Register Access Protocol, or URAP for short, is a lightweight,
fast and simple protocol for accessing registers through a variety of data
transmission standards, be it Unix Sockets, UART, RS-485, Ethernet IP or USB
Serial, URAP is simple to implement and open to be used by all.

## Protocol Overview
URAP is a data communication protocol where a primary client reads and writes
data to a secondary server, which allows access to up to 2^15 registers, each
32 bits wide. It assumes that it is being trasmitted on a one to one stream
where no other possible processes or devices can listen in or interpret the
packets; in other words when communicating over multi-device RS-485 or Ethernet
another transport protocol is required such as IPv4. It is not secure by any
means nor is it intended to be.

There are 5 types of packets and they are all dead simple:

 * Read packet, which has the WRITE bit set to zero, contain the WRITE bit,
 the 15 bit register address, and a CRC
 * Write packet, which has the WRITE bit set to one, contains the WRITE bit,
 the 15 bit register address, the data to write, and a CRC
 * Read ACK packet, contains an ACK byte, the data read, and a CRC
 * Write ACK packet, contains an ACK byte
 * NAK packet, containes a NAK byte

With these 5 packets you can have almost all the functionality you need for
simple embedded and non-embedded applications, where you just want to exchange
variables back and forth.

### Packet Structure

#### Read Packet

 1. **Bit 0**: Write bit, set to zero since we are reading
 2. **Bits 1-15**: Register number to read
 3. **Bits 16-23**: CRC of bits 0-15

#### Read-ACK Packet

 1. **Bits 0-7**: ACK byte, equal to 0xAA
 2. **Bits 8-39**: Data contained in the register
 3. **Bits 40-47**: CRC of bits 8-39

#### Write Packet

 1. **Bit 0**: Write bit, set to one since we are writing
 2. **Bits 1-15**: Register number to write to
 3. **Bits 16-47**: Data to write to the register
 3. **Bits 48-55**: CRC of bits 0-47

#### Write-ACK Packet

 1. **Bits 0-7**: ACK byte, equal to 0xAA

#### NAK Packet

 1. **Bits 0-7**: NAK byte, equal to 0x00. Any byte other than 0xAA should be
 handled as a NAK

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
```
1                                                                       Write Bit
 000 0000 0000 0000                                                     Register
                    0010 1010 0000 0000 0000 0000 0000 0000             Value
                                                            0000 1111   CRC
```

Secondary -> Primary, write-ack
```
1010 1010   ACK
```

Alternatively, if register 0 is write protected you will get a NAK


Secondary -> Primary, nak

```
0000 0000   NAK
```

#### Read Register 0

Primary -> Secondary, read register zero
```
0                               Write Bit
 000 0000 0000 0000             Register
                    0000 0000   CRC
```

Secondary -> Primary, read-ack with the value 42
```
1010 1010                                                       ACK
          0010 1010 0000 0000 0000 0000 0000 0000               Value
                                                  0100 1111     CRC
```
