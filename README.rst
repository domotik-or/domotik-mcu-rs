Building
========

Add the target:

.. code:: console

    rustup target add thumbv8m.main-none-eabihf

Build the target:

.. code:: console

    cargo build -release

Flashing
========

The chip is a STM32H523CCU6.

.. code:: console

    cargo flash --release --chip STM32H523CC

generating delivery files
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
