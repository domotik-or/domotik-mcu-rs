write configuration on SD card
==============================

cd config
./main.py --force --ip 192.168.1.59 --gateway 192.168.1.1 --mask 24 /dev/sde

dump the sd card after writing configuration
============================================

dd if=/dev/sde  bs=512 count=1 | od -t x1

udev rules
==========

Create file /etc/udev/rules.d/60-cmsis-daplink.rules :

# CMSIS-DAPLink debug probe
SUBSYSTEM=="usb", ATTR{idVendor}=="c251", ATTR{idProduct}=="f001", TAG+="uaccess"

# HID/CMSIS-DAPLink interface
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="c251", ATTRS{idProduct}=="f001", TAG+="uaccess"

sudo udevadm control --reload-rules
sudo udevadm trigger

Compile ST version of openocd
=============================

sudo apt install libusb-dev libhid-api libjim-dev
git clone --recurse-submodules https://github.com/STMicroelectronics/OpenOCD.git
cd OpenOCD/
./bootstrap
make
sudo make install

Use
===

debug :

/usr/local/bin/openocd -f debug/openocd.cfg
cargo run --release

flash :

cargo flash --release --chip STM32F401CC
