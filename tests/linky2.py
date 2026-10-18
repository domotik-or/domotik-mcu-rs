#!/usr/bin/env python3

import serial
import logging
from collections import deque

PORT = "/dev/ttyUSB0"
BAUDRATE = 9600

# Une variation supérieure à ce seuil est seulement signalée.
# La mesure n'est PAS rejetée.
ANOMALY_DELTA = 100

logging.basicConfig(
    filename="linky.log",
    level=logging.INFO,
    format="%(asctime)s.%(msecs)03d %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)

anomaly_log = logging.getLogger("anomalies")
handler = logging.FileHandler("linky-anomalies.log")
handler.setFormatter(logging.Formatter(
    "%(asctime)s.%(msecs)03d %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
))
anomaly_log.addHandler(handler)
anomaly_log.setLevel(logging.INFO)
anomaly_log.propagate = False


ser = serial.Serial(
    PORT,
    BAUDRATE,
    bytesize=serial.SEVENBITS,
    parity=serial.PARITY_EVEN,
    stopbits=serial.STOPBITS_ONE,
    timeout=1,
)


def checksum_ok(line):
    parts = line.split(b"\t")

    if len(parts) != 3:
        return False

    label, value, checksum = parts

    if len(checksum) != 1:
        return False

    data = label + b"\t" + value + b"\t"
    expected = (sum(data) & 0x3f) + 0x20

    return checksum[0] == expected


history = deque(maxlen=10)
previous = None

logging.info("START")


while True:
    raw = ser.readline()

    if not raw:
        continue

    line = raw.strip(b"\r\n")

    if not line.startswith(b"SINSTS\t"):
        continue

    if not checksum_ok(line):
        logging.warning("BAD CHECKSUM raw=%r", line)
        continue

    parts = line.split(b"\t")

    try:
        value = int(parts[1])
    except ValueError:
        logging.warning("BAD VALUE raw=%r", line)
        continue

    logging.info(
        "SINSTS=%d raw=%r",
        value,
        line,
    )

    if previous is not None:
        delta = abs(value - previous)

        if delta >= ANOMALY_DELTA:
            anomaly_log.warning(
                "SINSTS=%d previous=%d delta=%d raw=%r history=%s",
                value,
                previous,
                delta,
                line,
                list(history),
            )

    history.append(value)
    previous = value
