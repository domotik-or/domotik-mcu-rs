flashing
========

STM32F411CEU6

.. code:: console

    cargo flash --release --chip STM32F411CE

generating delivery files
=========================

.. code:: console

    cargo objcopy --release -- -O ihex delivery/pans-rs.hex
    cargo metadata --format-version 1 | js-beautify > delivery/metadata.json
