Installing
==========

Add the target (M4 + Fpu):

.. code:: console

    rustup target add thumbv7em-none-eabihf

write configuration on SD card
==============================

.. code:: console

    cd config
    ./main.py --force --ip 192.168.1.59 --gateway 192.168.1.1 --mask 24 /dev/sde

dump the sd card after writing configuration
============================================

.. code:: console

    dd if=/dev/sde  bs=512 count=1 | od -t x1

udev rules
==========

Create file /etc/udev/rules.d/60-cmsis-daplink.rules :

.. code::

    # CMSIS-DAPLink debug probe
    SUBSYSTEM=="usb", ATTR{idVendor}=="c251", ATTR{idProduct}=="f001", MODE="0660", GROUP="plugdev"

    # HID/CMSIS-DAPLink interface
    SUBSYSTEM=="hidraw", ATTRS{idVendor}=="c251", ATTRS{idProduct}=="f001", MODE="0660", GROUP="plugdev"

.. code:: console

    sudo udevadm control --reload-rules
    sudo udevadm trigger

Compile ST version of openocd
=============================

.. code:: console

    sudo apt install libusb-dev libhid-api libjim-dev
    git clone --recurse-submodules https://github.com/STMicroelectronics/OpenOCD.git
    cd OpenOCD/
    ./bootstrap
    make
    sudo make install

Use
---

debug :

.. code:: console

    /usr/local/bin/openocd -f debug/openocd.cfg

Cargo
=====

.. code:: console

    cargo build --release
    cargo run --release

Flash
-----

The chip is a STM32F401CC.

.. code:: console

    cargo flash --release --chip STM32F401CC

Run
---

.. code:: console

    probe-rs run --chip STM32F401CC --probe c251:f001 --scan-region ram --reset target/thumbv7em-none-eabihf/release/domotik-mcu
    probe-rs attach --chip STM32F401CC --probe c251:f001 target/thumbv7em-none-eabihf/release/domotik-mcu

Generating delivery files
=========================

Package to be installed : jsbeautifier.

.. code:: console

    cargo objcopy --release -- -O ihex delivery/pans-rs.hex
    cargo metadata --format-version 1 | js-beautify > delivery/metadata.json

Testing the libraries
=====================

.. code:: console

    cargo test --package my_libs --target x86_64-unknown-linux-gnu

Debugging the libraries
=======================

.. code:: console

    cargo test --no-run --package my_libs --target x86_64-unknown-linux-gnu
    gdb target/x86_64-unknown-linux-gnu/debug/deps/my_libs-6e93679ae5c2141c
    (gdb) break my_libs::linky::Linky::decode_frame
    (gdb) run
