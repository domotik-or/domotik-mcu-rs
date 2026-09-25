import random
import time

import serial


def main():
    with serial.Serial("/dev/ttyUSB0", 9600, parity=serial.PARITY_EVEN, bytesize=serial.EIGHTBITS) as ser:
        try:
            while True:
                temp = random.gauss(18.0, 0.25)
                hum = random.gauss(54.0, 0.25)
                press = random.gauss(974.0, 0.25)

                # ttttt,hhhhh,pppppp
                # 0     6     12
                values = f"{temp * 0100:05.0f},{hum * 100:05.0f},{press * 100:06.0f}"
                values = values.encode()
                checksum = (sum(values) & 0x3f) + 0x20
                buf = values + bytes([checksum]) + b"\r\n"
                ser.write(buf)
                time.sleep(3)
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
