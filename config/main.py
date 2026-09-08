#!/usr/bin/env python3

import argparse
import ipaddress
import os
import struct
import sys
import zlib

SECTOR_SIZE = 512

# "CFG1" in ASCII, stored as little-endian u32.
MAGIC = 0x43464731


def make_config(ip: str, gateway: str, mask: int) -> bytes:
    ip_addr = ipaddress.IPv4Address(ip)
    gateway_addr = ipaddress.IPv4Address(gateway)

    if not 0 <= mask <= 32:
        raise ValueError("mask must be between 0 and 32")

    sector = bytearray(SECTOR_SIZE)

    # Header
    struct.pack_into("<I", sector, 0, MAGIC)

    # Configuration
    sector[4:8] = ip_addr.packed
    sector[8:12] = gateway_addr.packed
    sector[12] = mask

    # CRC32 over magic + configuration.
    crc = zlib.crc32(sector[0:13]) & 0xFFFFFFFF
    struct.pack_into("<I", sector, 13, crc)

    return bytes(sector)


def main():
    parser = argparse.ArgumentParser(
        description="Write Ethernet configuration to the first SD-card sector."
    )

    parser.add_argument(
        "device",
        help="SD card device/file (e.g. /dev/sdb or sdcard.img)",
    )

    parser.add_argument(
        "--ip",
        required=True,
        help="Device IPv4 address, e.g. 192.168.1.50",
    )

    parser.add_argument(
        "--gateway",
        required=True,
        help="Gateway IPv4 address, e.g. 192.168.1.1",
    )

    parser.add_argument(
        "--mask",
        required=True,
        type=int,
        help="Network prefix length, e.g. 24",
    )

    parser.add_argument(
        "--force",
        action="store_true",
        help="Allow writing to a block device without confirmation.",
    )

    args = parser.parse_args()

    try:
        data = make_config(
            args.ip,
            args.gateway,
            args.mask,
        )
    except ValueError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    if os.path.exists(args.device):
        if os.path.basename(args.device).startswith("sd") and not args.force:
            print(
                f"WARNING: {args.device} looks like a block device.",
                file=sys.stderr,
            )
            print(
                "Use --force if you really want to write it.",
                file=sys.stderr,
            )
            return 1

    try:
        with open(args.device, "r+b", buffering=0) as f:
            f.seek(0)
            f.write(data)
            f.flush()
            os.fsync(f.fileno())

    except PermissionError:
        print(
            f"Permission denied writing {args.device}.",
            file=sys.stderr,
        )
        return 1

    except OSError as e:
        print(f"Write failed: {e}", file=sys.stderr)
        return 1

    magic = struct.unpack_from("<I", data, 0)[0]
    crc = struct.unpack_from("<I", data, 13)[0]

    print("Configuration written successfully:")
    print(f"  Magic:   0x{magic:08X}")
    print(f"  IP:      {args.ip}")
    print(f"  Gateway: {args.gateway}")
    print(f"  Mask:    /{args.mask}")
    print(f"  CRC32:   0x{crc:08X}")
    print(f"  Sector:  0")
    print(f"  Size:    {SECTOR_SIZE} bytes")

    print()
    print("Configuration bytes:")
    print(" ".join(f"{b:02X}" for b in data[:17]))

    return 0


if __name__ == "__main__":
    sys.exit(main())
